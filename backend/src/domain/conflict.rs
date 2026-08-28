use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    #[default]
    Ask,
    Replace,
    Skip,
    Rename,
    Fail,
}

pub struct ConflictResolver;

impl ConflictResolver {
    pub fn resolve_collision(
        policy: ConflictPolicy,
        target_name: &str,
    ) -> Result<Option<String>, AppError> {
        match policy {
            ConflictPolicy::Fail => Err(AppError::Conflict(format!(
                "Destination '{}' already exists",
                target_name
            ))),
            ConflictPolicy::Skip => Ok(None),
            ConflictPolicy::Replace => Ok(Some(target_name.to_string())),
            ConflictPolicy::Rename => {
                let parts: Vec<&str> = target_name.rsplitn(2, '.').collect();
                let new_name = if parts.len() == 2 {
                    format!("{}_copy.{}", parts[1], parts[0])
                } else {
                    format!("{}_copy", target_name)
                };
                Ok(Some(new_name))
            }
            ConflictPolicy::Ask => Ok(Some(target_name.to_string())),
        }
    }
}
