//! Application layer — thin orchestrators between API and Domain/Infrastructure (§85-88, §161).
//! API may know HTTP, Application knows domain + ports, Domain knows nothing about HTTP/SQL/Tokio.

pub mod files;

// Re-exports for handlers
pub use files::FileApplicationService;
