use crate::auth::AuthenticatedUser;
use crate::domain::operation::{
    FailureStrategy, OperationExecutionResult, OperationIntentType, OperationPlan,
};
use crate::domain::path::VfsPath;
use crate::domain::policy::PermissionInheritanceMode;
use crate::errors::AppError;
use crate::filesystem::archive::ArchiveOverwriteMode;
use crate::services::authorization_service::AuthorizationService;
use crate::services::file_service::FileService;
use crate::state::AppState;
use uuid::Uuid;

pub struct OperationService;

impl OperationService {
    #[allow(clippy::too_many_arguments)]
    pub fn create_plan(
        intent_type: OperationIntentType,
        source_connection_id: String,
        source_paths: Vec<VfsPath>,
        destination_connection_id: Option<String>,
        destination_path: Option<VfsPath>,
        failure_strategy: FailureStrategy,
        permission_mode: PermissionInheritanceMode,
        overwrite_mode: Option<ArchiveOverwriteMode>,
    ) -> OperationPlan {
        let id = format!("plan_{}", &Uuid::new_v4().to_string()[..8]);
        OperationPlan {
            id,
            intent_type,
            source_connection_id,
            source_paths,
            destination_connection_id,
            destination_path,
            failure_strategy,
            permission_mode,
            overwrite_mode,
        }
    }

    /// Authorize and execute an operation plan across providers
    pub async fn execute_plan(
        state: &AppState,
        user: &AuthenticatedUser,
        plan: &OperationPlan,
    ) -> Result<OperationExecutionResult, AppError> {
        // 1. Authorize operation intent
        AuthorizationService::authorize_intent(
            &state.db,
            user,
            plan.intent_type,
            &plan.source_connection_id,
            plan.destination_connection_id.as_deref(),
        )
        .await?;

        let mut result = OperationExecutionResult::new(plan.id.clone(), plan.source_paths.len());

        // 2. Dispatch execution per item with chosen failure strategy
        for path in &plan.source_paths {
            let res = match plan.intent_type {
                OperationIntentType::Delete => {
                    FileService::delete_entry(state, user, &plan.source_connection_id, &path.path)
                        .await
                }
                OperationIntentType::Move => {
                    if let Some(dest_p) = &plan.destination_path {
                        FileService::rename_entry(
                            state,
                            user,
                            &plan.source_connection_id,
                            &path.path,
                            &dest_p.path,
                        )
                        .await
                    } else {
                        Err(AppError::BadRequest(
                            "Destination path required for move".into(),
                        ))
                    }
                }
                OperationIntentType::Chmod => {
                    FileService::chmod(state, user, &plan.source_connection_id, &path.path, 0o644)
                        .await
                }
                _ => Ok(()),
            };

            match res {
                Ok(_) => {
                    result.succeeded_items.push(path.path.clone());
                }
                Err(e) => {
                    result.failed_items.push((path.path.clone(), e.to_string()));
                    if plan.failure_strategy == FailureStrategy::FailFast {
                        break;
                    }
                }
            }
        }

        result.finalize();
        Ok(result)
    }
}
