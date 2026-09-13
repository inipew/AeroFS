use crate::auth::AuthenticatedUser;
use crate::domain::{Actor, ConnectionId, FileKind, VfsPath};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::FileMutationEffects,
    filesystem::FileSystemResolver,
    trash::{NewTrashRecord, TrashRepository},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct TrashItem {
    pub id: String,
    pub connection_id: String,
    pub original_path: String,
    pub item_name: String,
    pub is_directory: bool,
    pub size: Option<i64>,
    pub deleted_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveToTrashRequest {
    pub connection_id: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MovedTrashItem {
    pub id: String,
    pub original_path: String,
}

#[derive(Clone)]
pub struct TrashService {
    repository: Arc<dyn TrashRepository>,
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileMutationEffects>,
}

impl TrashService {
    pub fn new(
        repository: Arc<dyn TrashRepository>,
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        effects: Arc<dyn FileMutationEffects>,
    ) -> Self {
        Self {
            repository,
            authorization,
            filesystem,
            effects,
        }
    }

    fn actor(user: &AuthenticatedUser) -> Actor {
        Actor {
            id: user.id.clone(),
            username: user.username.clone(),
            is_admin: user.is_admin,
        }
    }

    pub async fn list_trash(&self, _user: &AuthenticatedUser) -> Result<Vec<TrashItem>, AppError> {
        Ok(self
            .repository
            .list()
            .await?
            .into_iter()
            .map(|record| TrashItem {
                id: record.id,
                connection_id: record.connection_id,
                original_path: record.original_path,
                item_name: record.item_name,
                is_directory: record.is_directory,
                size: record.size,
                deleted_at: record.deleted_at,
            })
            .collect())
    }

    pub async fn move_to_trash(
        &self,
        user: &AuthenticatedUser,
        payload: MoveToTrashRequest,
    ) -> Result<Vec<MovedTrashItem>, AppError> {
        let connection = ConnectionId::new(payload.connection_id.clone())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let actor = Self::actor(user);
        self.authorization
            .authorize(&actor, &connection, FileAction::Write)
            .await?;
        let provider = self.filesystem.resolve(&connection).await?;

        let trash_dir_vfs = VfsPath::new(connection.as_str(), "/.trash")?;
        // Creating an existing provider directory is expected to be idempotent for
        // supported backends; any real provider error must abort the operation.
        provider.create_dir(&trash_dir_vfs).await?;

        let now_str = Utc::now().to_rfc3339();
        let mut moved_items = Vec::new();

        for path_str in &payload.paths {
            let vfs_path = VfsPath::new(connection.as_str(), path_str)?;
            let meta = provider.stat(&vfs_path).await?;
            let item_id = Uuid::new_v4().to_string();
            let trash_filename = format!("/.trash/{}_{}", &item_id[..8], meta.name);
            let dest_vfs = VfsPath::new(connection.as_str(), &trash_filename)?;

            provider.rename(&vfs_path, &dest_vfs).await?;

            let record = NewTrashRecord {
                id: item_id.clone(),
                connection_id: connection.as_str().to_string(),
                original_path: path_str.clone(),
                trash_path: trash_filename,
                item_name: meta.name,
                is_directory: meta.kind == FileKind::Directory,
                size: meta.size as i64,
                deleted_at: now_str.clone(),
                deleted_by: user.username.clone(),
            };

            if let Err(error) = self.repository.insert(&record).await {
                if let Err(rollback_error) = provider.rename(&dest_vfs, &vfs_path).await {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "trash persistence failed after filesystem rename and rollback failed; recovery required: persistence error: {}; rollback error: {}",
                        error,
                        rollback_error
                    )));
                }
                return Err(error);
            }

            self.effects
                .file_changed(
                    &actor,
                    &connection,
                    path_str,
                    "TRASH_MOVE",
                    "delete",
                    Some(format!("Moved {} to trash", path_str)),
                )
                .await?;

            moved_items.push(MovedTrashItem {
                id: item_id,
                original_path: path_str.clone(),
            });
        }

        Ok(moved_items)
    }

    pub async fn restore_item(
        &self,
        user: &AuthenticatedUser,
        trash_id: &str,
    ) -> Result<(), AppError> {
        let record = self
            .repository
            .get(trash_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Trash item not found".into()))?;
        let connection = ConnectionId::new(record.connection_id)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let actor = Self::actor(user);
        self.authorization
            .authorize(&actor, &connection, FileAction::Create)
            .await?;
        self.authorization
            .authorize(&actor, &connection, FileAction::Write)
            .await?;
        let provider = self.filesystem.resolve(&connection).await?;
        let trash_vfs = VfsPath::new(connection.as_str(), &record.trash_path)?;
        let orig_vfs = VfsPath::new(connection.as_str(), &record.original_path)?;
        provider.rename(&trash_vfs, &orig_vfs).await?;

        match self.repository.delete(trash_id).await {
            Ok(true) => {}
            Ok(false) => {
                if let Err(rollback_error) = provider.rename(&orig_vfs, &trash_vfs).await {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "trash record disappeared after filesystem restore and rollback failed; recovery required: {}",
                        rollback_error
                    )));
                }
                return Err(AppError::NotFound("Trash item not found".into()));
            }
            Err(error) => {
                if let Err(rollback_error) = provider.rename(&orig_vfs, &trash_vfs).await {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "trash persistence failed after filesystem restore and rollback failed; recovery required: persistence error: {}; rollback error: {}",
                        error,
                        rollback_error
                    )));
                }
                return Err(error);
            }
        }

        self.effects
            .file_changed(
                &actor,
                &connection,
                &record.original_path,
                "TRASH_RESTORE",
                "create",
                Some(format!("Restored {} from trash", record.original_path)),
            )
            .await?;
        Ok(())
    }

    pub async fn delete_permanently(
        &self,
        user: &AuthenticatedUser,
        trash_id: &str,
    ) -> Result<(), AppError> {
        if !user.is_admin {
            return Err(AppError::Forbidden(
                "Only administrators can permanently delete items from trash".into(),
            ));
        }

        let record = self
            .repository
            .get(trash_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Trash item not found".into()))?;
        let connection = ConnectionId::new(record.connection_id)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let provider = self.filesystem.resolve(&connection).await?;
        let trash_vfs = VfsPath::new(connection.as_str(), &record.trash_path)?;

        // Never discard the durable record when the physical delete failed.
        provider.delete(&trash_vfs).await?;
        if !self.repository.delete(trash_id).await? {
            return Err(AppError::Internal(anyhow::anyhow!(
                "trash file was permanently deleted but its durable record disappeared concurrently; recovery required"
            )));
        }
        Ok(())
    }

    pub async fn empty_trash(&self, user: &AuthenticatedUser) -> Result<usize, AppError> {
        let records = self.repository.list().await?;
        let actor = Self::actor(user);
        let mut deleted_count = 0usize;

        for record in records {
            let connection = ConnectionId::new(record.connection_id.clone())
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
            self.authorization
                .authorize(&actor, &connection, FileAction::Delete)
                .await?;
            let provider = self.filesystem.resolve(&connection).await?;
            let trash_vfs = VfsPath::new(connection.as_str(), &record.trash_path)?;

            provider.delete(&trash_vfs).await?;
            if !self.repository.delete(&record.id).await? {
                return Err(AppError::Internal(anyhow::anyhow!(
                    "trash file '{}' was deleted but its durable record disappeared concurrently; recovery required",
                    record.id
                )));
            }
            deleted_count += 1;
        }

        Ok(deleted_count)
    }
}
