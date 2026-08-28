use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::domain::operation::OperationIntentType;
use crate::errors::AppError;

pub struct AuthorizationService;

impl AuthorizationService {
    pub async fn can_read(
        db: &DbPool,
        user: &AuthenticatedUser,
        connection_id: &str,
    ) -> Result<(), AppError> {
        check_permission(db, user, connection_id, PermissionAction::Read).await
    }

    pub async fn can_write(
        db: &DbPool,
        user: &AuthenticatedUser,
        connection_id: &str,
    ) -> Result<(), AppError> {
        check_permission(db, user, connection_id, PermissionAction::Write).await
    }

    pub async fn can_create(
        db: &DbPool,
        user: &AuthenticatedUser,
        connection_id: &str,
    ) -> Result<(), AppError> {
        check_permission(db, user, connection_id, PermissionAction::Create).await
    }

    pub async fn can_delete(
        db: &DbPool,
        user: &AuthenticatedUser,
        connection_id: &str,
    ) -> Result<(), AppError> {
        check_permission(db, user, connection_id, PermissionAction::Delete).await
    }

    pub async fn authorize_intent(
        db: &DbPool,
        user: &AuthenticatedUser,
        intent: OperationIntentType,
        source_connection_id: &str,
        destination_connection_id: Option<&str>,
    ) -> Result<(), AppError> {
        match intent {
            OperationIntentType::Copy => {
                Self::can_read(db, user, source_connection_id).await?;
                if let Some(dest_conn) = destination_connection_id {
                    Self::can_create(db, user, dest_conn).await?;
                    Self::can_write(db, user, dest_conn).await?;
                }
            }
            OperationIntentType::Move => {
                Self::can_read(db, user, source_connection_id).await?;
                Self::can_delete(db, user, source_connection_id).await?;
                if let Some(dest_conn) = destination_connection_id {
                    Self::can_create(db, user, dest_conn).await?;
                    Self::can_write(db, user, dest_conn).await?;
                }
            }
            OperationIntentType::Delete => {
                Self::can_delete(db, user, source_connection_id).await?;
            }
            OperationIntentType::Chmod => {
                Self::can_write(db, user, source_connection_id).await?;
            }
            OperationIntentType::Compress => {
                Self::can_read(db, user, source_connection_id).await?;
                if let Some(dest_conn) = destination_connection_id {
                    Self::can_create(db, user, dest_conn).await?;
                    Self::can_write(db, user, dest_conn).await?;
                }
            }
            OperationIntentType::Extract => {
                Self::can_read(db, user, source_connection_id).await?;
                if let Some(dest_conn) = destination_connection_id {
                    Self::can_create(db, user, dest_conn).await?;
                    Self::can_write(db, user, dest_conn).await?;
                }
            }
        }
        Ok(())
    }
}
