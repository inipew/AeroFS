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
    audit::{SqliteAuditRepository, SqliteUserPreferencesRepository},
    auth::{SqliteAccountRepository, SqliteAuthAudit, SqliteSessionRepository},
    connection_runtime::{RegistryConnectionRuntime, RuntimeConnectionEffects},
    connections::SqliteConnectionRepository,
    files::{
        RegistryFileSystemResolver, SqliteAuthorization, SqliteConnectionStorageMetadata,
        SqliteFileMutationEffects, SqliteFileSettings,
    },
    health::RuntimeReadinessProbe,
    realtime::SqliteRealtimeAuthorization,
    settings::{RegistrySettingsRuntime, SqliteSettingsAudit, SqliteSystemSettingsStore},
    share::SqliteShareRepository,
    transfers::{SqliteTransferControl, SqliteTransferEffects, TransferEngineQueue},
    trash::SqliteTrashRepository,
    uploads::TransferUploadExecution,
    CredentialStore,
};
use crate::runtime::{ResourceBudget, RuntimeMetricsCollector};
use crate::services::{
    ArchiveService, AuditService, AuthService, ConnectionService, FileApiService, HealthService,
    PreferencesService, RealtimeService, SearchService, SettingsService, ShareService, SyncService,
    TrashService, UserService,
};
use crate::state::{
    AppState, ArchiveState, AuditState, AuthState, ConnectionState, FileApiState, HealthState,
    PreferencesState, RealtimeState, RouterState, RuntimeOwner, RuntimeState, SearchState,
    SettingsState, ShareState, SyncState, TransferState, TrashState,
};
use crate::sync::{SyncEventSubscriber, SyncManager};
use crate::transfer::{TransferEngine, TransferManager};
use crate::vfs::registry::ProviderRegistry;
use std::sync::Arc;

const REMOTE_PROVIDER_IDLE_TTL_SECS: u64 = 10 * 60;
const REMOTE_PROVIDER_REAPER_INTERVAL_SECS: u64 = 60;

pub struct BuiltApplication {
    pub state: AppState,
    pub runtime: RuntimeOwner,
}

pub fn build_user_service(db: DbPool) -> UserService {
    UserService::new(Arc::new(SqliteAccountRepository::new(db)))
}

pub async fn build_application(config: AppConfig, db: DbPool) -> BuiltApplication {
    let is_dev = config
        .get_by_key_path("aero_env")
        .map(|v| v == "development")
        .unwrap_or_else(|| {
            std::env::var("AEROFS_ENV").unwrap_or_else(|_| "development".into()) == "development"
                || cfg!(test)
        });
    let allowed_origins = config.security.allowed_origins.clone();
    let trusted_proxies = config.security.trusted_proxies.clone();
    let cookie_secure = config.security.cookie_secure;
    let session_ttl_secs = config.security.session_ttl_secs;

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
    let runtime = RuntimeOwner::default();
    let event_journal = Arc::new(
        EventJournal::init(db.clone())
            .await
            .expect("Failed to initialize durable event journal"),
    );

    let resource_budget = Arc::new(ResourceBudget::with_limits(
        config.limits.global_io_concurrency,
        config.limits.global_io_concurrency,
        config.limits.global_io_concurrency,
        config.limits.max_concurrent_transfers,
        config.limits.archive_concurrency,
        config.limits.search_concurrency,
    ));

    let transfer_manager = TransferManager::new(
        registry.providers_map(),
        db.clone(),
        config.limits.max_concurrent_transfers,
        resource_budget.clone(),
        event_journal.clone(),
        runtime.shutdown_token.clone(),
        &runtime.task_tracker,
    )
    .await;
    let transfer_engine = TransferEngine::with_shutdown(
        transfer_manager.clone(),
        runtime.shutdown_token.clone(),
    );
    let upload_execution = Arc::new(TransferUploadExecution::new(
        transfer_manager.clone(),
        resource_budget.clone(),
    ));

    let sync_manager = Arc::new(SyncManager::new(
        db.clone(),
        transfer_manager.clone(),
        runtime.supervisor.clone(),
        resource_budget.clone(),
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
    let runtime_metrics = RuntimeMetricsCollector::new(
        resource_budget.clone(),
        runtime.supervisor.clone(),
        registry.clone(),
        metadata_cache.clone(),
        db.clone(),
    );

    let upload_locks = Arc::new(crate::services::UploadLockManager::default());
    let max_upload_size = config.limits.max_upload_size;
    let local_root = config.filesystem.default_local_root.clone();
    let config = Arc::new(config);

    let file_authorization = Arc::new(SqliteAuthorization::new(db.clone()));
    let file_filesystem = Arc::new(RegistryFileSystemResolver::new(registry.clone()));
    let file_settings = Arc::new(SqliteFileSettings::new(db.clone(), config.clone()));
    let connection_storage = Arc::new(SqliteConnectionStorageMetadata::new(db.clone()));
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
        read_file: ReadFile::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_effects.clone(),
        ),
        write_file: WriteFile::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_settings.clone(),
            file_effects.clone(),
            upload_locks.clone(),
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
        upload_locks.clone(),
        upload_locks.clone(),
        upload_execution,
        file_settings.clone(),
        max_upload_size,
    );

    let settings_service = SettingsService::new(
        config.clone(),
        Arc::new(SqliteSystemSettingsStore::new(db.clone())),
        Arc::new(RegistrySettingsRuntime::new(
            registry.clone(),
            transfer_manager.clone(),
        )),
        Arc::new(SqliteSettingsAudit::new(db.clone())),
    );
    let settings = SettingsState::new(settings_service);

    let file_api = FileApiState::new(
        files.clone(),
        uploads,
        FileApiService::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            file_effects.clone(),
            file_effects.clone(),
            file_settings.clone(),
            connection_storage,
            metadata_cache.clone(),
        ),
    );

    let transfers = TransferUseCases::new(
        CreateTransfer::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            Arc::new(TransferEngineQueue::new(transfer_engine)),
            Arc::new(SqliteTransferEffects::new(db.clone())),
        ),
        Arc::new(SqliteTransferControl::new(
            db.clone(),
            transfer_manager.clone(),
            upload_locks,
        )),
    );

    let connection_repository = Arc::new(SqliteConnectionRepository::new(db.clone(), credentials));
    let connection_runtime = Arc::new(RegistryConnectionRuntime::new(
        config.clone(),
        registry.clone(),
    ));
    let connection_effects = Arc::new(RuntimeConnectionEffects::new(
        transfer_manager.clone(),
        metadata_cache.clone(),
    ));
    let connection_service = ConnectionService::new(
        connection_repository,
        file_settings,
        connection_runtime,
        connection_effects,
    );
    connection_service
        .load_all_providers_from_db()
        .await
        .expect("Failed to load persisted storage connection state");

    let auth_accounts = Arc::new(SqliteAccountRepository::new(db.clone()));
    let auth_sessions = Arc::new(SqliteSessionRepository::new(db.clone()));
    let auth_audit = Arc::new(SqliteAuthAudit::new(db.clone()));

    let state = AppState {
        router: RouterState::new(is_dev, allowed_origins),
        runtime: RuntimeState::new(runtime.view()),
        auth: AuthState::new(AuthService::new(
            auth_accounts,
            auth_sessions,
            auth_audit,
            trusted_proxies,
            cookie_secure,
            session_ttl_secs,
        )),
        connections: ConnectionState::new(connection_service),
        file_api,
        transfers: TransferState::new(transfers),
        search: SearchState::new(SearchService::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            resource_budget.clone(),
        )),
        health: HealthState::with_metrics(
            HealthService::new(Arc::new(RuntimeReadinessProbe::new(
                db.clone(),
                local_root.clone(),
                registry.clone(),
                runtime.view(),
                runtime.supervisor.clone(),
            ))),
            runtime_metrics,
        ),
        realtime: RealtimeState::new(RealtimeService::new(
            Arc::new(SqliteRealtimeAuthorization::new(db.clone())),
            event_journal.clone(),
            runtime.shutdown_token.clone(),
        )),
        sync: SyncState::new(SyncService::new(
            file_authorization.clone(),
            sync_manager,
        )),
        archive: ArchiveState::new(ArchiveService::new(
            file_authorization.clone(),
            file_filesystem.clone(),
            Arc::new(SqliteArchiveEffects::new(
                db.clone(),
                transfer_manager,
            )),
            resource_budget,
        )),
        settings,
        audit: AuditState::new(AuditService::new(Arc::new(SqliteAuditRepository::new(
            db.clone(),
        )))),
        preferences: PreferencesState::new(PreferencesService::new(Arc::new(
            SqliteUserPreferencesRepository::new(db.clone()),
        ))),
        shares: ShareState::new(ShareService::new(
            Arc::new(SqliteShareRepository::new(db.clone())),
            file_authorization.clone(),
            file_filesystem.clone(),
        )),
        trash: TrashState::new(TrashService::new(
            Arc::new(SqliteTrashRepository::new(db.clone())),
            file_authorization,
            file_filesystem,
            file_effects,
        )),
    };

    spawn_runtime_tasks(&runtime, local_root, event_journal, db, registry);
    BuiltApplication { state, runtime }
}

fn spawn_runtime_tasks(
    runtime: &RuntimeOwner,
    local_root: std::path::PathBuf,
    journal: Arc<EventJournal>,
    housekeeping_db: DbPool,
    registry: Arc<ProviderRegistry>,
) {
    let provider_reaper_token = runtime.shutdown_token.clone();
    let provider_reaper_health = runtime.supervisor.clone();
    runtime
        .supervisor
        .spawn("provider_idle_reaper", async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                REMOTE_PROVIDER_REAPER_INTERVAL_SECS,
            ));
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = provider_reaper_token.cancelled() => break,
                    _ = interval.tick() => {
                        let reclaimed = registry
                            .reclaim_idle_connections(std::time::Duration::from_secs(
                                REMOTE_PROVIDER_IDLE_TTL_SECS,
                            ))
                            .await;
                        if !reclaimed.is_empty() {
                            tracing::debug!(connections = ?reclaimed, "Reclaimed idle remote providers");
                        }
                        provider_reaper_health.record_success("provider_idle_reaper");
                    }
                }
            }
        });

    let cleanup_token = runtime.shutdown_token.clone();
    let cleanup_health = runtime.supervisor.clone();
    runtime
        .supervisor
        .spawn("stale_staging_cleanup", async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                crate::config::EVENT_JOURNAL_VACUUM_SECS,
            ));
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = cleanup_token.cancelled() => break,
                    _ = interval.tick() => {
                        match crate::vfs::cleanup_stale_staging_files(
                            &local_root,
                            std::time::Duration::from_secs(crate::config::STAGING_RETENTION_SECS),
                        ).await {
                            Ok(_) => cleanup_health.record_success("stale_staging_cleanup"),
                            Err(error) => cleanup_health.record_failure("stale_staging_cleanup", error),
                        }
                    }
                }
            }
        });

    let vacuum_token = runtime.shutdown_token.clone();
    let vacuum_health = runtime.supervisor.clone();
    runtime
        .supervisor
        .spawn("event_journal_vacuum", async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                crate::config::EVENT_JOURNAL_VACUUM_SECS,
            ));
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = vacuum_token.cancelled() => break,
                    _ = interval.tick() => {
                        match journal
                            .vacuum(std::time::Duration::from_secs(
                                crate::config::EVENT_JOURNAL_RETENTION_SECS,
                            ))
                            .await
                        {
                            Ok(_) => vacuum_health.record_success("event_journal_vacuum"),
                            Err(error) => vacuum_health.record_failure("event_journal_vacuum", error),
                        }
                    }
                }
            }
        });

    let housekeeping_token = runtime.shutdown_token.clone();
    let housekeeping_health = runtime.supervisor.clone();
    runtime.supervisor.spawn("housekeeping", async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = housekeeping_token.cancelled() => break,
                _ = interval.tick() => {
                    let now = chrono::Utc::now().to_rfc3339();
                    let mut failed = false;

                    match sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
                        .bind(&now)
                        .execute(&housekeeping_db)
                        .await
                    {
                        Ok(result) => {
                            if result.rows_affected() > 0 {
                                tracing::info!("Housekeeping: purged {} expired sessions", result.rows_affected());
                            }
                        }
                        Err(error) => {
                            failed = true;
                            housekeeping_health.record_failure("housekeeping", error);
                        }
                    }

                    match sqlx::query(
                        "DELETE FROM transfer_jobs WHERE dismissed_at IS NOT NULL AND dismissed_at < datetime('now', '-30 days')",
                    )
                    .execute(&housekeeping_db)
                    .await
                    {
                        Ok(result) => {
                            if result.rows_affected() > 0 {
                                tracing::info!("Housekeeping: purged {} old dismissed transfer jobs", result.rows_affected());
                            }
                        }
                        Err(error) => {
                            failed = true;
                            housekeeping_health.record_failure("housekeeping", error);
                        }
                    }

                    if !failed {
                        housekeeping_health.record_success("housekeeping");
                    }
                }
            }
        }
    });
}
