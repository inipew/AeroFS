use crate::application::{
    files::{
        ChmodEntry, CompletePresigned, CopyEntry, CreateDirectory, DeleteEntries, FileUseCases,
        ListDirectory, PresignDownload, PresignUpload, ReadFile, RenameEntry, StatFile, WriteFile,
    },
    transfers::{CreateTransfer, TransferUseCases},
    UploadApplicationService,
};
use crate::config::AppConfig;
use crate::db::DbPool;
use crate::events::{EventJournal, MetadataCacheEventSubscriber};
use crate::infrastructure::{
    archive::SqliteArchiveEffects,
    files::{
        RegistryFileSystemResolver, SqliteAuthorization, SqliteFileMutationEffects,
        SqliteFileSettings,
    },
    transfers::{SqliteTransferControl, SqliteTransferEffects, TransferEngineQueue},
    CredentialStore,
};
use crate::runtime::ResourceBudget;
use crate::services::{
    connection_service::ConnectionService, ArchiveService, HealthService, RealtimeService,
    SearchService, SyncService,
};
use crate::state::{
    AppRuntime, AppState, ArchiveState, HealthState, RealtimeState, SearchState, SyncState,
};
use crate::sync::{SyncEventSubscriber, SyncManager};
use crate::transfer::{TransferEngine, TransferManager};
use crate::vfs::registry::ProviderRegistry;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Build the full application graph once at process startup.
/// This is the composition root: concrete adapters are selected here and are
/// injected into application use-cases through their ports.
pub async fn build_app_state(config: AppConfig, db: DbPool) -> AppState {
    let cred_key = config
        .security
        .credential_encryption_key
        .as_deref()
        .unwrap_or(&config.security.session_secret);
    if config.security.credential_encryption_key.is_none()
        && config.security.session_secret != "dev_secret_change_in_production_32_chars_min"
    {
        tracing::warn!("Using session_secret for credential encryption — set AEROFS_CREDENTIAL_ENCRYPTION_KEY for key separation");
    }

    let credentials = Arc::new(CredentialStore::new(cred_key));
    let registry = Arc::new(ProviderRegistry::new());
    let runtime = AppRuntime::default();
    let event_journal = Arc::new(
        EventJournal::init(db.clone())
            .await
            .expect("Failed to initialize durable event journal"),
    );
    let resource_budget = Arc::new(ResourceBudget::default());

    let transfer_manager = TransferManager::new(
        registry.providers_map(),
        db.clone(),
        config.limits.max_concurrent_transfers,
        event_journal.clone(),
        runtime.shutdown_token.clone(),
        &runtime.task_tracker,
    )
    .await;
    let transfer_engine = TransferEngine::new(transfer_manager.clone());

    let sync_manager = Arc::new(SyncManager::new(
        db.clone(),
        transfer_manager.clone(),
        runtime.supervisor.clone(),
        event_journal.clone(),
        registry.providers_map(),
    ));
    SyncEventSubscriber::spawn(
        &runtime.supervisor,
        event_journal.clone(),
        sync_manager.clone(),
        runtime.shutdown_token.clone(),
    );

    let metadata_cache = Arc::new(crate::services::MetadataCache::default());
    MetadataCacheEventSubscriber::spawn(
        &runtime.supervisor,
        event_journal.clone(),
        metadata_cache.clone(),
        runtime.shutdown_token.clone(),
    );

    let upload_locks = Arc::new(crate::services::UploadLockManager::default());
    let cfg_limits_global = config.limits.global_io_concurrency;
    let cfg_limits_archive = config.limits.archive_concurrency;
    let cfg_limits_search = config.limits.search_concurrency;
    let max_editable_size = config.limits.max_editable_size;
    let max_upload_size = config.limits.max_upload_size;
    let local_root = config.filesystem.default_local_root.clone();
    let config = Arc::new(config);

    let file_authorization = Arc::new(SqliteAuthorization::new(db.clone()));
    let file_filesystem = Arc::new(RegistryFileSystemResolver::new(registry.clone()));
    let file_settings = Arc::new(SqliteFileSettings::new(db.clone(), config.clone()));
    let file_effects = Arc::new(SqliteFileMutationEffects::new(
        db.clone(),
        metadata_cache.clone(),
        event_journal.clone(),
    ));

    let files = FileUseCases {
        list_directory: ListDirectory::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_settings.clone(),
        ),
        stat_file: StatFile::new(file_authorization.clone(), file_filesystem.clone()),
        read_file: ReadFile::new(file_authorization.clone(), file_filesystem.clone()),
        write_file: WriteFile::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_settings,
            file_effects.clone(),
        ),
        create_directory: CreateDirectory::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_effects.clone(),
        ),
        rename_entry: RenameEntry::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_effects.clone(),
        ),
        copy_entry: CopyEntry::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_effects.clone(),
        ),
        delete_entries: DeleteEntries::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_effects.clone(),
        ),
        presign_download: PresignDownload::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_effects.clone(),
        ),
        presign_upload: PresignUpload::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_effects.clone(),
        ),
        complete_presigned: CompletePresigned::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_effects.clone(),
        ),
        chmod_entry: ChmodEntry::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_effects.clone(),
        ),
    };

    let uploads = UploadApplicationService::new(
        file_authorization.clone(),
        file_filesystem.clone(),
        file_effects.clone(),
        transfer_manager.clone(),
        upload_locks.clone(),
        local_root.clone(),
        max_editable_size,
        max_upload_size,
    );

    let search = SearchState::new(SearchService::new(
        file_authorization.clone(),
        file_filesystem.clone(),
        Arc::new(Semaphore::new(cfg_limits_search)),
    ));

    let health = HealthState::new(HealthService::new(
        db.clone(),
        local_root,
        registry.clone(),
        runtime.view(),
    ));

    let realtime = RealtimeState::new(RealtimeService::new(
        db.clone(),
        event_journal.clone(),
        runtime.shutdown_token.clone(),
    ));

    let sync = SyncState::new(SyncService::new(
        file_authorization.clone(),
        sync_manager.clone(),
    ));

    let archive = ArchiveState::new(ArchiveService::new(
        file_authorization.clone(),
        file_filesystem.clone(),
        Arc::new(SqliteArchiveEffects::new(
            db.clone(),
            transfer_manager.clone(),
        )),
        Arc::new(Semaphore::new(cfg_limits_archive)),
    ));

    let transfers = TransferUseCases::new(
        CreateTransfer::new(
            file_authorization,
            file_filesystem,
            Arc::new(TransferEngineQueue::new(transfer_engine.clone())),
            Arc::new(SqliteTransferEffects::new(db.clone())),
        ),
        Arc::new(SqliteTransferControl::new(
            db.clone(),
            transfer_manager.clone(),
            upload_locks.clone(),
        )),
    );

    let state = AppState {
        config,
        db,
        registry,
        credentials,
        transfer_manager,
        transfer_engine,
        metadata_cache,
        upload_locks,
        global_io_semaphore: Arc::new(Semaphore::new(cfg_limits_global)),
        resource_budget,
        event_journal,
        sync_manager,
        runtime,
        files,
        transfers,
        uploads,
        search,
        health,
        realtime,
        sync,
        archive,
    };

    ConnectionService::load_all_providers_from_db(&state).await;
    spawn_runtime_tasks(&state);
    state
}

fn spawn_runtime_tasks(state: &AppState) {
    let local_root = state.config.filesystem.default_local_root.clone();
    let cleanup_token = state.runtime.shutdown_token.clone();
    state.runtime.supervisor.spawn("stale_staging_cleanup", async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            crate::config::EVENT_JOURNAL_VACUUM_SECS,
        ));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = cleanup_token.cancelled() => break,
                _ = interval.tick() => {
                    let _ = crate::vfs::cleanup_stale_staging_files(
                        &local_root,
                        std::time::Duration::from_secs(crate::config::STAGING_RETENTION_SECS),
                    ).await;
                }
            }
        }
    });

    let journal = state.event_journal.clone();
    let vacuum_token = state.runtime.shutdown_token.clone();
    state.runtime.supervisor.spawn("event_journal_vacuum", async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            crate::config::EVENT_JOURNAL_VACUUM_SECS,
        ));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = vacuum_token.cancelled() => break,
                _ = interval.tick() => {
                    let _ = journal
                        .vacuum(std::time::Duration::from_secs(
                            crate::config::EVENT_JOURNAL_RETENTION_SECS,
                        ))
                        .await;
                }
            }
        }
    });

    let housekeeping_db = state.db.clone();
    let housekeeping_token = state.runtime.shutdown_token.clone();
    state.runtime.supervisor.spawn("housekeeping", async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = housekeeping_token.cancelled() => break,
                _ = interval.tick() => {
                    let now = chrono::Utc::now().to_rfc3339();
                    if let Ok(result) = sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
                        .bind(&now)
                        .execute(&housekeeping_db)
                        .await
                    {
                        if result.rows_affected() > 0 {
                            tracing::info!("Housekeeping: purged {} expired sessions", result.rows_affected());
                        }
                    }

                    if let Ok(result) = sqlx::query(
                        "DELETE FROM transfer_jobs WHERE dismissed_at IS NOT NULL AND dismissed_at < datetime('now', '-30 days')",
                    )
                    .execute(&housekeeping_db)
                    .await
                    {
                        if result.rows_affected() > 0 {
                            tracing::info!("Housekeeping: purged {} old dismissed transfer jobs", result.rows_affected());
                        }
                    }
                }
            }
        }
    });
}
