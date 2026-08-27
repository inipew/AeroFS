pub mod audit;
pub mod credentials;
pub mod middleware;
pub mod password;
pub mod permissions;
pub mod session;

pub use audit::{record_audit_log, AuditLogEntry};
pub use credentials::{decrypt_secret, derive_master_key, encrypt_secret};
pub use middleware::AuthenticatedUser;
pub use password::{hash_password, verify_password};
pub use permissions::{check_permission, PermissionAction};
pub use session::{create_session, delete_session, validate_session, UserInfo};
