use crate::auth::audit::record_audit_log;
use crate::auth::password::verify_password;
use crate::auth::session::{create_session, delete_session, validate_session, UserInfo};
use crate::db::DbPool;
use crate::errors::{AppError, AuthError};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

static FAILED_ATTEMPTS: LazyLock<Mutex<HashMap<String, Vec<Instant>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
pub struct AuthService {
    db: DbPool,
    trusted_proxies: Vec<String>,
    cookie_secure: bool,
    session_ttl_secs: u64,
}

impl AuthService {
    pub fn new(
        db: DbPool,
        trusted_proxies: Vec<String>,
        cookie_secure: bool,
        session_ttl_secs: u64,
    ) -> Self {
        Self {
            db,
            trusted_proxies,
            cookie_secure,
            session_ttl_secs,
        }
    }

    pub fn trusted_proxies(&self) -> &[String] {
        &self.trusted_proxies
    }

    pub fn cookie_secure(&self) -> bool {
        self.cookie_secure
    }

    pub fn session_ttl_secs(&self) -> u64 {
        self.session_ttl_secs
    }

    pub async fn validate_session(&self, session_id: &str) -> Result<Option<UserInfo>, AppError> {
        validate_session(&self.db, session_id).await
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        client_ip: &str,
    ) -> Result<(UserInfo, String), AppError> {
        let user_key = format!("user:{}", username.trim().to_lowercase());
        let ip_key = format!("ip:{}", client_ip);

        let now = Instant::now();
        let window = Duration::from_secs(60);
        const MAX_FAILED_PER_USER: usize = 5;
        const MAX_FAILED_PER_IP: usize = 20;

        {
            if let Ok(mut map) = FAILED_ATTEMPTS.lock() {
                if let Some(ip_attempts) = map.get_mut(&ip_key) {
                    ip_attempts.retain(|t| now.duration_since(*t) < window);
                    if ip_attempts.len() >= MAX_FAILED_PER_IP {
                        return Err(AppError::Forbidden(
                            "Too many failed login attempts from this network. Please wait 60 seconds before trying again."
                                .into(),
                        ));
                    }
                }

                if let Some(user_attempts) = map.get_mut(&user_key) {
                    user_attempts.retain(|t| now.duration_since(*t) < window);
                    if user_attempts.len() >= MAX_FAILED_PER_USER {
                        return Err(AppError::Forbidden(
                            "Too many failed login attempts for this account. Please wait 60 seconds before trying again."
                                .into(),
                        ));
                    }
                }
            }
        }

        let row: Option<(String, String, String, i64)> = sqlx::query_as(
            "SELECT id, username, password_hash, is_admin FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database query error: {}", e))?;

        let (user_id, valid_username, password_hash, is_admin) = match row {
            Some(r) => r,
            None => {
                if let Ok(mut map) = FAILED_ATTEMPTS.lock() {
                    map.entry(ip_key).or_default().push(now);
                }
                record_audit_log(
                    &self.db,
                    None,
                    "AUTH_LOGIN_FAILED",
                    None,
                    None,
                    "FAILURE",
                    Some(client_ip),
                    Some(&format!("User not found: {}", username)),
                )
                .await;
                return Err(AppError::Auth(AuthError::InvalidCredentials));
            }
        };

        if !verify_password(password, &password_hash) {
            if let Ok(mut map) = FAILED_ATTEMPTS.lock() {
                map.entry(ip_key).or_default().push(now);
                map.entry(user_key).or_default().push(now);
            }
            record_audit_log(
                &self.db,
                Some(&user_id),
                "AUTH_LOGIN_FAILED",
                None,
                None,
                "FAILURE",
                Some(client_ip),
                Some("Invalid password"),
            )
            .await;
            return Err(AppError::Auth(AuthError::InvalidCredentials));
        }

        if let Ok(mut map) = FAILED_ATTEMPTS.lock() {
            map.remove(&user_key);
            map.remove(&ip_key);
        }

        let session_id = create_session(&self.db, &user_id, self.session_ttl_secs).await?;

        record_audit_log(
            &self.db,
            Some(&user_id),
            "AUTH_LOGIN_SUCCESS",
            None,
            None,
            "SUCCESS",
            Some(client_ip),
            Some(&format!("Logged in: {}", valid_username)),
        )
        .await;

        Ok((
            UserInfo {
                id: user_id,
                username: valid_username,
                is_admin: is_admin != 0,
            },
            session_id,
        ))
    }

    pub async fn logout(
        &self,
        session_id: &str,
        user_id: Option<&str>,
        client_ip: &str,
    ) -> Result<(), AppError> {
        delete_session(&self.db, session_id).await?;
        record_audit_log(
            &self.db,
            user_id,
            "AUTH_LOGOUT",
            None,
            None,
            "SUCCESS",
            Some(client_ip),
            Some("User logged out"),
        )
        .await;
        Ok(())
    }
}
