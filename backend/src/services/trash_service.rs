use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::domain::{Actor, ConnectionId, FileKind, VfsPath};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::FileMutationEffects,
    filesystem::FileSystemResolver,
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

type TrashDbRow = (String, String, String, String, i64, Option<i64>, String);

#[derive(Clone)]
pub struct TrashService {
    db: DbPool,
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileMutationEffects>,
}

impl TrashService {
    pub fn new(
        db: DbPool,
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        effects: Arc<dyn FileMutationEffects>,
    ) -> Self {
        Self {
            db,
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
        let rows: Vec<TrashDbRow> = sqlx::query_as(
            "SELECT id, connection_id, original_path, item_name, is_directory, size, deleted_at FROM trash_items ORDER BY deleted_at DESC",
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        Ok(rows
            .into_iter()
            .map(
                |(id, connection_id, original_path, item_name, is_dir, size, deleted_at)| {
                    TrashItem {
                        id,
                        connection_id,
                        original_path,
                        item_name,
                        is_directory: is_dir != 0,
                        size,
                        deleted_at,
                    }
                },
            )
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

        let now_str = Utc::now().to_rfc3339();
        let trash_dir_vfs = VfsPath::new(connection.as_str(), "/.trash")?;
        let _ = provider.create_dir(&trash_dir_vfs).await;
        let mut moved_items = Vec::new();

        for path_str in &payload.paths {
            let vfs_path = match VfsPath::new(connection.as_str(), path_str) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Ok(meta) = provider.stat(&vfs_path).await {
                let item_id = Uuid::new_v4().to_string();
                let trash_filename = format!("/.trash/{}_{}", &item_id[..8], meta.name);
                let dest_vfs = match VfsPath::new(connection.as_str(), &trash_filename) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if provider.rename(&vfs_path, &dest_vfs).await.is_ok() {
                    let is_dir = i64::from(meta.kind == FileKind::Directory);
                    let inserted = sqlx::query(
                        "INSERT INTO trash_items (id, connection_id, original_path, trash_path, item_name, is_directory, size, deleted_at, deleted_by)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&item_id)
                    .bind(connection.as_str())
                    .bind(path_str)
                    .bind(&trash_filename)
                    .bind(&meta.name)
                    .bind(is_dir)
                    .bind(meta.size as i64)
                    .bind(&now_str)
                    .bind(&user.username)
                    .execute(&self.db)
                    .await;

                    if inserted.is_err() {
                        let _ = provider.rename(&dest_vfs, &vfs_path).await;
                        continue;
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
            }
        }

        Ok(moved_items)
    }

    pub async fn restore_item(
        &self,
        user: &AuthenticatedUser,
        trash_id: &str,
    ) -> Result<(), AppError> {
        let row: Option<(String, String, String)> = sqlx::query_as(
            "SELECT connection_id, original_path, trash_path FROM trash_items WHERE id = ?",
        )
        .bind(trash_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        let (connection_id, orig_path, trash_path) =
            row.ok_or_else(|| AppError::NotFound("Trash item not found".into()))?;
        let connection = ConnectionId::new(connection_id)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let actor = Self::actor(user);
        self.authorization
            .authorize(&actor, &connection, FileAction::Create)
            .await?;
        self.authorization
            .authorize(&actor, &connection, FileAction::Write)
            .await?;
        let provider = self.filesystem.resolve(&connection).await?;
        let trash_vfs = VfsPath::new(connection.as_str(), &trash_path)?;
        let orig_vfs = VfsPath::new(connection.as_str(), &orig_path)?;
        provider.rename(&trash_vfs, &orig_vfs).await?;

        if let Err(error) = sqlx::query("DELETE FROM trash_items WHERE id = ?")
            .bind(trash_id)
            .execute(&self.db)
            .await
        {
            let _ = provider.rename(&orig_vfs, &trash_vfs).await;
            return Err(anyhow::anyhow!("Database error: {}", error).into());
        }

        self.effects
            .file_changed(
                &actor,
                &connection,
                &orig_path,
                "TRASH_RESTORE",
                "create",
                Some(format!("Restored {} from trash", orig_path)),
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
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT connection_id, trash_path FROM trash_items WHERE id = ?")
                .bind(trash_id)
                .fetch_optional(&self.db)
                .await
                .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        let (connection_id, trash_path) =
            row.ok_or_else(|| AppError::NotFound("Trash item not found".into()))?;
        let connection = ConnectionId::new(connection_id)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        if let Ok(provider) = self.filesystem.resolve(&connection).await {
            if let Ok(trash_vfs) = VfsPath::new(connection.as_str(), &trash_path) {
                let _ = provider.delete(&trash_vfs).await;
            }
        }
        sqlx::query("DELETE FROM trash_items WHERE id = ?")
            .bind(trash_id)
            .execute(&self.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        Ok(())
    }

    pub async fn empty_trash(&self, user: &AuthenticatedUser) -> Result<usize, AppError> {
        let rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, connection_id, trash_path FROM trash_items")
                .fetch_all(&self.db)
                .await
                .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        let actor = Self::actor(user);
        let mut deleted_count = 0;
        for (id, connection_id, trash_path) in rows {
            let connection = match ConnectionId::new(connection_id) {
                Ok(connection) => connection,
                Err(_) => continue,
            };
            if self
                .authorization
                .authorize(&actor, &connection, FileAction::Delete)
                .await
                .is_err()
            {
                continue;
            }
            if let Ok(provider) = self.filesystem.resolve(&connection).await {
                if let Ok(trash_vfs) = VfsPath::new(connection.as_str(), &trash_path) {
                    let _ = provider.delete(&trash_vfs).await;
                }
            }
            if sqlx::query("DELETE FROM trash_items WHERE id = ?")
                .bind(&id)
                .execute(&self.db)
                .await
                .is_ok()
            {
                deleted_count += 1;
            }
        }
        Ok(deleted_count)
    }
}
