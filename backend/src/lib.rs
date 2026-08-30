#![allow(clippy::too_many_arguments, clippy::match_like_matches_macro)]
#![allow(clippy::needless_borrow)]
#![allow(dead_code)]

pub mod api;
pub mod application;
pub mod auth;
pub mod cli;
pub mod config;
pub mod db;
pub mod domain;
pub mod errors;
pub mod events;
pub mod filesystem;
pub mod infrastructure;
pub mod middleware;
pub mod router;
pub mod runtime;
pub mod security;
pub mod services;
pub mod state;
pub mod static_files;
pub mod sync;
pub mod transfer;
pub mod vfs;

pub use router::create_router;
pub use state::AppState;
