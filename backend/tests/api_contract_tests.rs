mod support;

#[path = "api_contract/authentication.rs"]
mod authentication;
#[path = "api_contract/authorization.rs"]
mod authorization;
#[path = "api_contract/files.rs"]
mod files;
#[path = "api_contract/http.rs"]
mod http;
#[path = "api_contract/middleware.rs"]
mod middleware;
#[path = "api_contract/public_access.rs"]
mod public_access;
#[path = "api_contract/static_assets.rs"]
mod static_assets;
