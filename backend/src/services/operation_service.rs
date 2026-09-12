use crate::auth::AuthenticatedUser;
use crate::domain::operation::{
    FailureStrategy, OperationExecutionResult, OperationIntentType, OperationPlan,
};
use crate::domain::path::VfsPath;
use crate::domain::policy::PermissionInheritanceMode;
use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use crate::filesystem::archive::ArchiveOverwriteMode;
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

    pub async fn execute_plan(
        state: &AppState,
        user: &AuthenticatedUser,
        plan: &OperationPlan,
    ) -> Result<OperationExecutionResult, AppError> {
        let actor = Actor {
            id: user.id.clone(),
            username: user.username.clone(),
            is_admin: user.is_admin,
        };
        let source = ConnectionId::new(plan.source_connection_id.clone())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let destination = match plan.destination_connection_id.as_ref() {
            Some(id) => Some(
                ConnectionId::new(id.clone()).map_err(|e| AppError::BadRequest(e.to_string()))?,
            ),
            None => None,
        };
        state
            .file_api
            .service
            .authorize_intent(&actor, plan.intent_type, &source, destination.as_ref())
            .await?;

        let mut result = OperationExecutionResult::new(plan.id.clone(), plan.source_paths.len());
        for path in &plan.source_paths {
            let res = match plan.intent_type {
                OperationIntentType::Delete => {
                    let delete_result = state
                        .file_api
                        .files
                        .delete_entries
                        .execute(
                            &actor,
                            crate::application::files::DeleteEntriesCommand {
                                connection: source.clone(),
                                paths: vec![path.path.clone()],
                            },
                        )
                        .await?;
                    if let Some((_, error)) = delete_result.failed.first() {
                        Err(AppError::Internal(anyhow::anyhow!(error.clone())))
                    } else if delete_result.succeeded.is_empty() {
                        Err(AppError::NotFound(format!("{} not found", path.path)))
                    } else {
                        Ok(())
                    }
                }
                OperationIntentType::Move => {
                    if let Some(dest_p) = &plan.destination_path {
                        state
                            .file_api
                            .files
                            .rename_entry
                            .execute(
                                &actor,
                                crate::application::files::RenameEntryCommand {
                                    connection: source.clone(),
                                    from: path.path.clone(),
                                    to: dest_p.path.clone(),
                                },
                            )
                            .await
                    } else {
                        Err(AppError::BadRequest(
                            "Destination path required for move".into(),
                        ))
                    }
                }
                OperationIntentType::Chmod => {
                    state
                        .file_api
                        .files
                        .chmod_entry
                        .execute(
                            &actor,
                            crate::application::files::ChmodEntryCommand {
                                connection: source.clone(),
                                path: path.path.clone(),
                                mode: 0o644,
                            },
                        )
                        .await
                }
                _ => Ok(()),
            };

            match res {
                Ok(_) => result.succeeded_items.push(path.path.clone()),
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
