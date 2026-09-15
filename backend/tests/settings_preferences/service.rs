use async_trait::async_trait;
use backend::{
    config::AppConfig,
    domain::{settings::AppSettings, Actor},
    errors::AppError,
    ports::settings::{PreparedLocalRoot, SettingsAudit, SettingsRuntime, SystemSettingsStore},
    services::settings_service::{SettingsService, UpdateSettingsRequest},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct RecordingStore {
    values: Mutex<HashMap<String, String>>,
    events: Arc<Mutex<Vec<String>>>,
    fail_get: bool,
    fail_upsert: bool,
}

impl RecordingStore {
    fn new(events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            events,
            ..Self::default()
        }
    }

    fn with_get_failure(events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            events,
            fail_get: true,
            ..Self::default()
        }
    }

    fn with_upsert_failure(events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            events,
            fail_upsert: true,
            ..Self::default()
        }
    }

    fn value(&self, key: &str) -> Option<String> {
        self.values.lock().unwrap().get(key).cloned()
    }
}

#[async_trait]
impl SystemSettingsStore for RecordingStore {
    async fn get(&self, key: &str) -> Result<Option<String>, AppError> {
        if self.fail_get {
            return Err(AppError::Internal(anyhow::anyhow!("injected settings read failure")));
        }
        Ok(self.values.lock().unwrap().get(key).cloned())
    }

    async fn upsert_many(&self, values: &[(String, String)]) -> Result<(), AppError> {
        self.events.lock().unwrap().push("persist".to_string());
        if self.fail_upsert {
            return Err(AppError::Internal(anyhow::anyhow!("injected settings write failure")));
        }
        let mut stored = self.values.lock().unwrap();
        for (key, value) in values {
            stored.insert(key.clone(), value.clone());
        }
        Ok(())
    }
}

struct RecordingPreparedRoot {
    events: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl PreparedLocalRoot for RecordingPreparedRoot {
    async fn activate(self: Box<Self>) {
        self.events.lock().unwrap().push("activate".to_string());
    }
}

struct RecordingRuntime {
    events: Arc<Mutex<Vec<String>>>,
    fail_prepare: bool,
}

impl RecordingRuntime {
    fn new(events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            events,
            fail_prepare: false,
        }
    }

    fn failing(events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            events,
            fail_prepare: true,
        }
    }
}

#[async_trait]
impl SettingsRuntime for RecordingRuntime {
    async fn prepare_local_root(
        &self,
        new_root: PathBuf,
    ) -> Result<Box<dyn PreparedLocalRoot>, AppError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("prepare:{}", new_root.display()));
        if self.fail_prepare {
            return Err(AppError::Internal(anyhow::anyhow!("injected prepare failure")));
        }
        Ok(Box::new(RecordingPreparedRoot {
            events: self.events.clone(),
        }))
    }

    fn set_max_concurrent_transfers(&self, limit: usize) {
        self.events
            .lock()
            .unwrap()
            .push(format!("limit:{limit}"));
    }
}

struct RecordingAudit {
    events: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl SettingsAudit for RecordingAudit {
    async fn settings_updated(&self, actor: &Actor) {
        self.events
            .lock()
            .unwrap()
            .push(format!("audit:{}", actor.username));
    }
}

fn actor(is_admin: bool) -> Actor {
    Actor {
        id: if is_admin { "admin-id" } else { "user-id" }.to_string(),
        username: if is_admin { "admin" } else { "regular" }.to_string(),
        is_admin,
    }
}

fn service(
    store: Arc<RecordingStore>,
    runtime: Arc<RecordingRuntime>,
    events: Arc<Mutex<Vec<String>>>,
) -> SettingsService {
    SettingsService::new(
        Arc::new(AppConfig::default()),
        store,
        runtime,
        Arc::new(RecordingAudit { events }),
    )
}

#[tokio::test]
async fn update_orders_prepare_commit_activate_runtime_limit_and_audit() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = Arc::new(RecordingStore::new(events.clone()));
    let runtime = Arc::new(RecordingRuntime::new(events.clone()));
    let service = service(store.clone(), runtime, events.clone());

    let mut settings = AppSettings::default();
    settings.connections.default_local_root = "/new-root".to_string();
    settings.transfers.max_concurrent_transfers = 7;
    settings.general.theme = "light".to_string();

    service
        .update_settings(
            &actor(true),
            UpdateSettingsRequest {
                settings: Some(settings),
                local_root: None,
                temp_dir: None,
                allow_symlinks: None,
                show_hidden_default: None,
                read_only_default: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(
        events.lock().unwrap().as_slice(),
        [
            "prepare:/new-root",
            "persist",
            "activate",
            "limit:7",
            "audit:admin",
        ]
    );
    assert_eq!(store.value("local_root").as_deref(), Some("/new-root"));
    assert_eq!(store.value("theme").as_deref(), Some("light"));
    assert_eq!(store.value("max_concurrent_transfers").as_deref(), Some("7"));
}

#[tokio::test]
async fn non_admin_update_is_rejected_before_any_side_effect() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = Arc::new(RecordingStore::new(events.clone()));
    let runtime = Arc::new(RecordingRuntime::new(events.clone()));
    let service = service(store, runtime, events.clone());

    let result = service
        .update_settings(
            &actor(false),
            UpdateSettingsRequest {
                settings: None,
                local_root: Some("/forbidden".to_string()),
                temp_dir: None,
                allow_symlinks: None,
                show_hidden_default: None,
                read_only_default: None,
            },
        )
        .await;

    assert!(matches!(result, Err(AppError::Forbidden(_))));
    assert!(events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn prepare_failure_prevents_persistence_activation_and_audit() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = Arc::new(RecordingStore::new(events.clone()));
    let runtime = Arc::new(RecordingRuntime::failing(events.clone()));
    let service = service(store, runtime, events.clone());

    let result = service
        .update_settings(
            &actor(true),
            UpdateSettingsRequest {
                settings: None,
                local_root: Some("/bad-root".to_string()),
                temp_dir: None,
                allow_symlinks: None,
                show_hidden_default: None,
                read_only_default: None,
            },
        )
        .await;

    assert!(result.is_err());
    assert_eq!(events.lock().unwrap().as_slice(), ["prepare:/bad-root"]);
}

#[tokio::test]
async fn persistence_failure_does_not_activate_prepared_root_or_audit() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = Arc::new(RecordingStore::with_upsert_failure(events.clone()));
    let runtime = Arc::new(RecordingRuntime::new(events.clone()));
    let service = service(store, runtime, events.clone());

    let result = service
        .update_settings(
            &actor(true),
            UpdateSettingsRequest {
                settings: None,
                local_root: Some("/root".to_string()),
                temp_dir: None,
                allow_symlinks: None,
                show_hidden_default: None,
                read_only_default: None,
            },
        )
        .await;

    assert!(result.is_err());
    assert_eq!(events.lock().unwrap().as_slice(), ["prepare:/root", "persist"]);
}

#[tokio::test]
async fn settings_reads_propagate_store_failures() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = Arc::new(RecordingStore::with_get_failure(events.clone()));
    let runtime = Arc::new(RecordingRuntime::new(events.clone()));
    let service = service(store, runtime, events);

    let result = service.get_settings(&actor(true)).await;
    assert!(matches!(result, Err(AppError::Internal(_))));
}

#[tokio::test]
async fn settings_reads_use_config_defaults_when_store_has_no_override() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = Arc::new(RecordingStore::new(events.clone()));
    let runtime = Arc::new(RecordingRuntime::new(events.clone()));
    let config = AppConfig::default();
    let expected_root = config
        .filesystem
        .default_local_root
        .to_string_lossy()
        .to_string();
    let expected_limit = config.limits.max_concurrent_transfers;
    let service = SettingsService::new(
        Arc::new(config),
        store,
        runtime,
        Arc::new(RecordingAudit { events }),
    );

    let response = service.get_settings(&actor(false)).await.unwrap();
    assert_eq!(response.local_root, expected_root);
    assert_eq!(response.settings.general.theme, "dark");
    assert_eq!(
        response.settings.transfers.max_concurrent_transfers,
        expected_limit
    );
}
