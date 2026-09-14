use async_trait::async_trait;
use backend::{
    domain::{Capabilities, Connection},
    errors::AppError,
    ports::{
        connections::{
            ConnectionEffects, ConnectionRepository, ConnectionRuntime, ConnectionRuntimeInfo,
            ConnectionSecretError, DetachedConnectionRuntime, PreparedConnectionRuntime,
            SecretMutation,
        },
        settings::FileSettings,
    },
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub type EventLog = Arc<Mutex<Vec<String>>>;

pub fn event_log() -> EventLog {
    Arc::new(Mutex::new(Vec::new()))
}

#[derive(Debug, Clone)]
pub enum SecretFailure {
    Persistence(String),
    Decryption(String),
}

#[derive(Default)]
pub struct RecordingConnectionRepository {
    connections: Mutex<HashMap<String, Connection>>,
    secrets: Mutex<HashMap<String, String>>,
    readable: Mutex<HashSet<(String, String)>>,
    secret_failures: Mutex<HashMap<String, SecretFailure>>,
    delete_failure: Mutex<Option<String>>,
    events: EventLog,
}

impl RecordingConnectionRepository {
    pub fn new(events: EventLog) -> Self {
        Self {
            events,
            ..Self::default()
        }
    }

    pub fn insert_connection(&self, connection: Connection) {
        self.connections
            .lock()
            .unwrap()
            .insert(connection.id.clone(), connection);
    }

    pub fn set_secret(&self, id: &str, secret: &str) {
        self.secrets
            .lock()
            .unwrap()
            .insert(id.to_string(), secret.to_string());
    }

    pub fn secret(&self, id: &str) -> Option<String> {
        self.secrets.lock().unwrap().get(id).cloned()
    }

    pub fn allow_read(&self, user_id: &str, id: &str) {
        self.readable
            .lock()
            .unwrap()
            .insert((user_id.to_string(), id.to_string()));
    }

    pub fn fail_secret(&self, id: &str, failure: SecretFailure) {
        self.secret_failures
            .lock()
            .unwrap()
            .insert(id.to_string(), failure);
    }

    pub fn fail_delete_once(&self, message: &str) {
        *self.delete_failure.lock().unwrap() = Some(message.to_string());
    }
}

#[async_trait]
impl ConnectionRepository for RecordingConnectionRepository {
    async fn local_root_override(&self) -> Result<Option<PathBuf>, AppError> {
        Ok(None)
    }

    async fn load_enabled(&self) -> Result<Vec<Connection>, AppError> {
        Ok(self
            .connections
            .lock()
            .unwrap()
            .values()
            .filter(|connection| connection.enabled)
            .cloned()
            .collect())
    }

    async fn load_secret(&self, id: &str) -> Result<Option<String>, ConnectionSecretError> {
        if let Some(failure) = self.secret_failures.lock().unwrap().get(id).cloned() {
            return Err(match failure {
                SecretFailure::Persistence(message) => ConnectionSecretError::Persistence(message),
                SecretFailure::Decryption(message) => ConnectionSecretError::Decryption(message),
            });
        }
        Ok(self.secrets.lock().unwrap().get(id).cloned())
    }

    async fn can_read(&self, user_id: &str, id: &str) -> Result<bool, AppError> {
        Ok(self
            .readable
            .lock()
            .unwrap()
            .contains(&(user_id.to_string(), id.to_string())))
    }

    async fn list(&self, user_id: Option<&str>, is_admin: bool) -> Result<Vec<Connection>, AppError> {
        let readable = self.readable.lock().unwrap();
        Ok(self
            .connections
            .lock()
            .unwrap()
            .values()
            .filter(|connection| {
                is_admin
                    || user_id
                        .map(|user_id| readable.contains(&(user_id.to_string(), connection.id.clone())))
                        .unwrap_or(false)
            })
            .cloned()
            .collect())
    }

    async fn get(&self, id: &str) -> Result<Option<Connection>, AppError> {
        Ok(self.connections.lock().unwrap().get(id).cloned())
    }

    async fn exists(&self, id: &str) -> Result<bool, AppError> {
        Ok(self.connections.lock().unwrap().contains_key(id))
    }

    async fn create(&self, connection: &Connection, secret: Option<&str>) -> Result<(), AppError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("repository.create:{}", connection.id));
        self.connections
            .lock()
            .unwrap()
            .insert(connection.id.clone(), connection.clone());
        if let Some(secret) = secret {
            self.set_secret(&connection.id, secret);
        }
        Ok(())
    }

    async fn update(
        &self,
        connection: &Connection,
        secret: SecretMutation,
    ) -> Result<(), AppError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("repository.update:{}", connection.id));
        self.connections
            .lock()
            .unwrap()
            .insert(connection.id.clone(), connection.clone());
        match secret {
            SecretMutation::Keep => {}
            SecretMutation::Replace(secret) => self.set_secret(&connection.id, &secret),
            SecretMutation::Clear => {
                self.secrets.lock().unwrap().remove(&connection.id);
            }
        }
        Ok(())
    }

    async fn disable(&self, id: &str) -> Result<(), AppError> {
        let mut connections = self.connections.lock().unwrap();
        let connection = connections
            .get_mut(id)
            .ok_or_else(|| AppError::NotFound(format!("Connection '{id}' not found")))?;
        connection.enabled = false;
        Ok(())
    }

    async fn delete_with_transfer_barrier(&self, id: &str) -> Result<bool, AppError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("repository.delete_with_transfer_barrier:{id}"));
        if let Some(message) = self.delete_failure.lock().unwrap().take() {
            return Err(AppError::Internal(anyhow::anyhow!(message)));
        }
        self.secrets.lock().unwrap().remove(id);
        Ok(self.connections.lock().unwrap().remove(id).is_some())
    }
}

pub struct RecordingConnectionRuntime {
    events: EventLog,
    info: Arc<Mutex<HashMap<String, ConnectionRuntimeInfo>>>,
    prepared_secrets: Mutex<Vec<(String, Option<String>)>>,
    prepare_failures: Mutex<HashMap<String, String>>,
}

impl RecordingConnectionRuntime {
    pub fn new(events: EventLog) -> Self {
        Self {
            events,
            info: Arc::new(Mutex::new(HashMap::new())),
            prepared_secrets: Mutex::new(Vec::new()),
            prepare_failures: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_active(&self, id: &str, active: bool) {
        self.info.lock().unwrap().insert(
            id.to_string(),
            ConnectionRuntimeInfo {
                active,
                error_message: None,
                capabilities: Some(Capabilities::default()),
            },
        );
    }

    pub fn prepared_secrets(&self) -> Vec<(String, Option<String>)> {
        self.prepared_secrets.lock().unwrap().clone()
    }

    pub fn fail_prepare(&self, id: &str, message: &str) {
        self.prepare_failures
            .lock()
            .unwrap()
            .insert(id.to_string(), message.to_string());
    }
}

struct PreparedRecordingRuntime {
    id: String,
    events: EventLog,
    info: Arc<Mutex<HashMap<String, ConnectionRuntimeInfo>>>,
}

#[async_trait]
impl PreparedConnectionRuntime for PreparedRecordingRuntime {
    async fn activate(self: Box<Self>) {
        self.events
            .lock()
            .unwrap()
            .push(format!("runtime.activate:{}", self.id));
        self.info.lock().unwrap().insert(
            self.id.clone(),
            ConnectionRuntimeInfo {
                active: true,
                error_message: None,
                capabilities: Some(Capabilities::default()),
            },
        );
    }
}

struct DetachedRecordingRuntime {
    id: String,
    events: EventLog,
    info: Arc<Mutex<HashMap<String, ConnectionRuntimeInfo>>>,
    previous: ConnectionRuntimeInfo,
}

#[async_trait]
impl DetachedConnectionRuntime for DetachedRecordingRuntime {
    async fn restore(self: Box<Self>) {
        self.events
            .lock()
            .unwrap()
            .push(format!("runtime.restore:{}", self.id));
        self.info
            .lock()
            .unwrap()
            .insert(self.id.clone(), self.previous.clone());
    }
}

#[async_trait]
impl ConnectionRuntime for RecordingConnectionRuntime {
    async fn validate_target(&self, _host: Option<&str>, _port: Option<u16>) -> Result<(), AppError> {
        self.events.lock().unwrap().push("runtime.validate_target".into());
        Ok(())
    }

    async fn prepare_local(
        &self,
        _root: &Path,
    ) -> Result<Box<dyn PreparedConnectionRuntime>, AppError> {
        self.events.lock().unwrap().push("runtime.prepare:local".into());
        Ok(Box::new(PreparedRecordingRuntime {
            id: "local".into(),
            events: self.events.clone(),
            info: self.info.clone(),
        }))
    }

    async fn prepare(
        &self,
        connection: &Connection,
        secret: Option<&str>,
    ) -> Result<Box<dyn PreparedConnectionRuntime>, AppError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("runtime.prepare:{}", connection.id));
        self.prepared_secrets.lock().unwrap().push((
            connection.id.clone(),
            secret.map(str::to_string),
        ));
        if let Some(message) = self.prepare_failures.lock().unwrap().get(&connection.id).cloned() {
            return Err(AppError::BadRequest(message));
        }
        Ok(Box::new(PreparedRecordingRuntime {
            id: connection.id.clone(),
            events: self.events.clone(),
            info: self.info.clone(),
        }))
    }

    async fn set_error(&self, id: &str, error: &str) {
        self.events
            .lock()
            .unwrap()
            .push(format!("runtime.set_error:{id}"));
        self.info.lock().unwrap().insert(
            id.to_string(),
            ConnectionRuntimeInfo {
                active: false,
                error_message: Some(error.to_string()),
                capabilities: None,
            },
        );
    }

    async fn info(&self, id: &str) -> ConnectionRuntimeInfo {
        self.info
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or(ConnectionRuntimeInfo {
                active: false,
                error_message: None,
                capabilities: None,
            })
    }

    async fn detach(&self, id: &str) -> Option<Box<dyn DetachedConnectionRuntime>> {
        self.events
            .lock()
            .unwrap()
            .push(format!("runtime.detach:{id}"));
        let previous = self.info.lock().unwrap().remove(id)?;
        Some(Box::new(DetachedRecordingRuntime {
            id: id.to_string(),
            events: self.events.clone(),
            info: self.info.clone(),
            previous,
        }))
    }

    async fn remove(&self, id: &str) {
        self.events
            .lock()
            .unwrap()
            .push(format!("runtime.remove:{id}"));
        self.info.lock().unwrap().remove(id);
    }

    async fn test(&self, id: &str) -> Result<u64, AppError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("runtime.test:{id}"));
        Ok(7)
    }
}

pub struct RecordingConnectionEffects {
    events: EventLog,
    cancel_failure: Mutex<Option<String>>,
}

impl RecordingConnectionEffects {
    pub fn new(events: EventLog) -> Self {
        Self {
            events,
            cancel_failure: Mutex::new(None),
        }
    }

    pub fn fail_cancel_once(&self, message: &str) {
        *self.cancel_failure.lock().unwrap() = Some(message.to_string());
    }
}

#[async_trait]
impl ConnectionEffects for RecordingConnectionEffects {
    async fn cancel_active_transfers(&self, id: &str) -> Result<(), AppError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("effects.cancel_active_transfers:{id}"));
        if let Some(message) = self.cancel_failure.lock().unwrap().take() {
            return Err(AppError::Internal(anyhow::anyhow!(message)));
        }
        Ok(())
    }

    async fn invalidate_metadata(&self, id: &str) {
        self.events
            .lock()
            .unwrap()
            .push(format!("effects.invalidate_metadata:{id}"));
    }
}

pub struct StaticConnectionSettings {
    root: PathBuf,
}

impl StaticConnectionSettings {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[async_trait]
impl FileSettings for StaticConnectionSettings {
    async fn show_hidden_default(&self) -> Result<bool, AppError> {
        Ok(false)
    }

    async fn max_editable_size(&self) -> Result<u64, AppError> {
        Ok(1024 * 1024)
    }

    async fn local_root(&self) -> Result<PathBuf, AppError> {
        Ok(self.root.clone())
    }

    async fn allow_symlinks_outside_root(&self) -> Result<bool, AppError> {
        Ok(false)
    }

    fn max_directory_entries(&self) -> usize {
        10_000
    }
}
