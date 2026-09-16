use async_trait::async_trait;
use axum::extract::FromRef;
use backend::{
    application::files::{
        CreateDirectory, CreateDirectoryCommand, WriteFile, WriteFileCommand,
    },
    config::AppConfig,
    domain::{Actor, Capabilities, ConnectionId, VfsPath},
    errors::AppError,
    infrastructure::files::SqliteFileSettings,
    ports::{
        authorization::{Authorization, FileAction},
        cache::FileMetadataCache,
        effects::FileMutationEffects,
        mutation::{MutationCoordinator, MutationLease},
        settings::FileSettings,
    },
    services::cache::MetadataCache,
    state::FileApiState,
    vfs::FileSystem,
};
use std::{path::PathBuf, sync::Arc};

use crate::support::{MemoryFileSystem, SwappableFileSystemResolver, TestAppBuilder, TestDatabase};

#[derive(Default)]
struct AllowAllAuthorization;

#[async_trait]
impl Authorization for AllowAllAuthorization {
    async fn authorize(
        &self,
        _actor: &Actor,
        _connection: &ConnectionId,
        _action: FileAction,
    ) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Default)]
struct NoopEffects;

#[async_trait]
impl FileMutationEffects for NoopEffects {
    async fn invalidate(&self, _connection: &ConnectionId, _path: &str) {}
    async fn invalidate_prefix(&self, _connection: &ConnectionId, _path: &str) {}

    async fn file_changed(
        &self,
        _actor: &Actor,
        _connection: &ConnectionId,
        _path: &str,
        _audit_action: &'static str,
        _event_action: &'static str,
        _details: Option<String>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn file_renamed(
        &self,
        _actor: &Actor,
        _connection: &ConnectionId,
        _from: &str,
        _to: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn file_copied(
        &self,
        _actor: &Actor,
        _connection: &ConnectionId,
        _from: &str,
        _to: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Default)]
struct NoopMutations;

#[async_trait]
impl MutationCoordinator for NoopMutations {
    async fn try_acquire(
        &self,
        _connection: &ConnectionId,
        _path: &str,
    ) -> Result<Box<dyn MutationLease>, AppError> {
        Ok(Box::new(()))
    }
}

struct StaticSettings {
    max_editable_size: u64,
}

#[async_trait]
impl FileSettings for StaticSettings {
    async fn show_hidden_default(&self) -> Result<bool, AppError> {
        Ok(false)
    }

    async fn max_editable_size(&self) -> Result<u64, AppError> {
        Ok(self.max_editable_size)
    }

    async fn local_root(&self) -> Result<PathBuf, AppError> {
        Ok(std::env::temp_dir())
    }

    async fn allow_symlinks_outside_root(&self) -> Result<bool, AppError> {
        Ok(false)
    }

    fn max_directory_entries(&self) -> usize {
        10_000
    }
}

fn actor() -> Actor {
    Actor {
        id: "file-user".into(),
        username: "file-user".into(),
        is_admin: true,
    }
}

fn permission_provider(atomic_rename: bool) -> Arc<MemoryFileSystem> {
    Arc::new(MemoryFileSystem::new(Capabilities {
        stat: true,
        read: true,
        write: true,
        create_file: true,
        create_dir: true,
        delete: true,
        rename: true,
        permissions: true,
        atomic_rename,
        ..Default::default()
    }))
}

#[test]
fn metadata_cache_satisfies_the_application_cache_port_at_compile_time() {
    fn assert_cache_port<T: FileMetadataCache>() {}
    assert_cache_port::<MetadataCache>();
}

#[tokio::test]
async fn sqlite_file_settings_propagate_database_failure_instead_of_defaulting() {
    let database = TestDatabase::migrated("file_settings_failure.db").await;
    let settings = SqliteFileSettings::new(database.pool.clone(), Arc::new(AppConfig::default()));
    database.pool.close().await;

    let error = settings
        .max_editable_size()
        .await
        .expect_err("closed settings database must surface an error");
    assert!(matches!(error, AppError::Internal(_)));
}

#[tokio::test]
async fn recursive_chmod_rejects_remote_storage_before_provider_mutation() {
    let app = TestAppBuilder::new().running().build().await;
    let file_api = FileApiState::from_ref(&app.state);
    let error = file_api
        .service
        .chmod_recursive(
            &actor(),
            &ConnectionId::new("remote-test").unwrap(),
            "/anything",
            0o755,
        )
        .await
        .expect_err("recursive chmod must stay local-only");

    assert!(matches!(error, AppError::BadRequest(message) if message.contains("only supported for local storage")));
}

#[tokio::test]
async fn directory_permission_failure_reports_partial_commit_recovery_state() {
    let provider = permission_provider(false);
    let root = VfsPath::root("memory");
    provider.set_permissions(&root, "0750").await.unwrap();
    provider.fail_permissions("chmod denied");
    let provider_dyn: Arc<dyn FileSystem> = provider.clone();
    let resolver = Arc::new(SwappableFileSystemResolver::new(provider_dyn));
    let use_case = CreateDirectory::new(
        Arc::new(AllowAllAuthorization),
        resolver,
        Arc::new(NoopEffects),
    );

    let error = use_case
        .execute(
            &actor(),
            CreateDirectoryCommand {
                connection: ConnectionId::new("memory").unwrap(),
                path: "/created".into(),
            },
        )
        .await
        .expect_err("post-create chmod failure must be explicit");

    assert!(error
        .to_string()
        .contains("Filesystem mutation committed; recovery required"));
    assert_eq!(
        provider
            .stat(&VfsPath::new("memory", "/created").unwrap())
            .await
            .unwrap()
            .kind,
        backend::domain::FileKind::Directory
    );
}

#[tokio::test]
async fn staged_write_permission_failure_cleans_staging_without_committing_target() {
    let provider = permission_provider(true);
    provider
        .set_permissions(&VfsPath::root("memory"), "0750")
        .await
        .unwrap();
    provider.fail_permissions("chmod staging denied");
    let provider_dyn: Arc<dyn FileSystem> = provider.clone();
    let resolver = Arc::new(SwappableFileSystemResolver::new(provider_dyn));
    let use_case = WriteFile::new(
        Arc::new(AllowAllAuthorization),
        resolver,
        Arc::new(StaticSettings {
            max_editable_size: 1024,
        }),
        Arc::new(NoopEffects),
        Arc::new(NoopMutations),
    );

    let error = use_case
        .execute(
            &actor(),
            WriteFileCommand {
                connection: ConnectionId::new("memory").unwrap(),
                path: "/target.txt".into(),
                content: b"payload".to_vec(),
                expected_etag: None,
                create_only: false,
            },
        )
        .await
        .expect_err("staging chmod failure must abort before commit");
    assert!(error.to_string().contains("chmod staging denied"));
    assert!(provider
        .stat(&VfsPath::new("memory", "/target.txt").unwrap())
        .await
        .is_err());
    let entries = provider.list(&VfsPath::root("memory")).await.unwrap();
    assert!(entries
        .iter()
        .all(|entry| !entry.name.contains(".aerofs.tmp-")));
}

#[tokio::test]
async fn direct_write_permission_failure_reports_committed_recovery_state() {
    let provider = permission_provider(false);
    provider
        .set_permissions(&VfsPath::root("memory"), "0750")
        .await
        .unwrap();
    provider.fail_permissions("chmod target denied");
    let provider_dyn: Arc<dyn FileSystem> = provider.clone();
    let resolver = Arc::new(SwappableFileSystemResolver::new(provider_dyn));
    let use_case = WriteFile::new(
        Arc::new(AllowAllAuthorization),
        resolver,
        Arc::new(StaticSettings {
            max_editable_size: 1024,
        }),
        Arc::new(NoopEffects),
        Arc::new(NoopMutations),
    );

    let error = use_case
        .execute(
            &actor(),
            WriteFileCommand {
                connection: ConnectionId::new("memory").unwrap(),
                path: "/target.txt".into(),
                content: b"committed payload".to_vec(),
                expected_etag: None,
                create_only: false,
            },
        )
        .await
        .expect_err("direct chmod failure must expose committed state");
    assert!(error
        .to_string()
        .contains("Filesystem mutation committed; recovery required"));
    assert_eq!(provider.bytes("/target.txt").as_deref(), Some(b"committed payload".as_slice()));
}
