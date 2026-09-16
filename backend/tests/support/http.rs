use super::app::TestApp;
use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, HeaderValue, Method, Request, Response, StatusCode},
};
use backend::auth::{create_session, hash_password};
use chrono::Utc;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

const FIXTURE_SESSION_TTL_SECS: u64 = 60 * 60;

#[derive(Debug, Clone)]
pub struct TestUser {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct TestSession {
    pub cookie: String,
    pub token: String,
    pub set_cookie: String,
}

#[derive(Debug, Clone, Copy)]
pub enum TestAuth<'a> {
    Anonymous,
    Cookie(&'a TestSession),
    Bearer(&'a TestSession),
}

#[derive(Debug, Clone, Copy)]
pub struct PermissionGrant {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub delete: bool,
    pub rename: bool,
    pub upload: bool,
    pub download: bool,
}

impl PermissionGrant {
    pub const fn none() -> Self {
        Self {
            read: false,
            write: false,
            create: false,
            delete: false,
            rename: false,
            upload: false,
            download: false,
        }
    }

    pub const fn read_only() -> Self {
        Self {
            read: true,
            download: true,
            ..Self::none()
        }
    }

    pub const fn full() -> Self {
        Self {
            read: true,
            write: true,
            create: true,
            delete: true,
            rename: true,
            upload: true,
            download: true,
        }
    }
}

impl TestApp {
    pub async fn seed_user(&self, username: &str, password: &str, is_admin: bool) -> TestUser {
        let password_hash = hash_password(password).expect("hash fixture password");
        self.insert_fixture_user(username, password_hash, is_admin).await
    }

    /// Seed a user for tests that exercise authorization/session behavior rather than
    /// password hashing. Reuse the already-valid seeded admin hash so the fixture row
    /// remains structurally production-realistic without paying another Argon2 hash.
    pub async fn seed_session_user(&self, username: &str, is_admin: bool) -> TestUser {
        let password_hash: String =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE username = 'admin'")
                .fetch_one(&self.db)
                .await
                .expect("seeded admin password hash must exist");
        self.insert_fixture_user(username, password_hash, is_admin).await
    }

    async fn insert_fixture_user(
        &self,
        username: &str,
        password_hash: String,
        is_admin: bool,
    ) -> TestUser {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, is_admin, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(username)
        .bind(password_hash)
        .bind(i64::from(is_admin))
        .bind(&now)
        .bind(&now)
        .execute(&self.db)
        .await
        .expect("insert fixture user");

        TestUser {
            id,
            username: username.to_string(),
            is_admin,
        }
    }

    pub async fn grant_permissions(
        &self,
        user: &TestUser,
        connection_id: &str,
        grant: PermissionGrant,
    ) {
        sqlx::query(
            "INSERT INTO permissions (id, user_id, connection_id, can_read, can_write, can_create, can_delete, can_rename, can_upload, can_download) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(user_id, connection_id) DO UPDATE SET can_read=excluded.can_read, can_write=excluded.can_write, can_create=excluded.can_create, can_delete=excluded.can_delete, can_rename=excluded.can_rename, can_upload=excluded.can_upload, can_download=excluded.can_download",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&user.id)
        .bind(connection_id)
        .bind(i64::from(grant.read))
        .bind(i64::from(grant.write))
        .bind(i64::from(grant.create))
        .bind(i64::from(grant.delete))
        .bind(i64::from(grant.rename))
        .bind(i64::from(grant.upload))
        .bind(i64::from(grant.download))
        .execute(&self.db)
        .await
        .expect("upsert fixture permissions");
    }

    /// Exercise the real login endpoint, including password verification and response
    /// cookie attributes. Keep this for authentication-specific tests.
    pub async fn login_session(&self, username: &str, password: &str) -> TestSession {
        let response = self
            .json_request(
                Method::POST,
                "/api/v1/auth/login",
                TestAuth::Anonymous,
                Some(json!({"username": username, "password": password})),
            )
            .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "fixture login failed for {username}"
        );

        let set_cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .expect("login must set session cookie")
            .to_str()
            .expect("session cookie must be header text")
            .to_string();
        let cookie = set_cookie
            .split(';')
            .next()
            .expect("session cookie pair")
            .to_string();
        let token = cookie
            .strip_prefix("session_id=")
            .expect("session cookie name")
            .to_string();

        TestSession {
            cookie,
            token,
            set_cookie,
        }
    }

    /// Create a production session row directly for tests whose subject is downstream
    /// authorization or request behavior rather than password authentication.
    pub async fn session_for_user(&self, user: &TestUser) -> TestSession {
        let token = create_session(&self.db, &user.id, FIXTURE_SESSION_TTL_SECS)
            .await
            .expect("create fixture session");
        let cookie = format!("session_id={token}");
        TestSession {
            set_cookie: cookie.clone(),
            cookie,
            token,
        }
    }

    pub async fn admin_session(&self) -> TestSession {
        let (id, username, is_admin): (String, String, i64) = sqlx::query_as(
            "SELECT id, username, is_admin FROM users WHERE username = 'admin'",
        )
        .fetch_one(&self.db)
        .await
        .expect("seeded admin must exist");
        self.session_for_user(&TestUser {
            id,
            username,
            is_admin: is_admin != 0,
        })
        .await
    }

    pub async fn admin_cookie(&self) -> String {
        self.admin_session().await.cookie
    }

    pub async fn json_request(
        &self,
        method: Method,
        uri: &str,
        auth: TestAuth<'_>,
        payload: Option<Value>,
    ) -> Response<Body> {
        self.json_request_with_headers(method, uri, auth, HeaderMap::new(), payload)
            .await
    }

    pub async fn json_request_with_headers(
        &self,
        method: Method,
        uri: &str,
        auth: TestAuth<'_>,
        extra_headers: HeaderMap,
        payload: Option<Value>,
    ) -> Response<Body> {
        let has_payload = payload.is_some();
        let body = payload
            .map(|value| Body::from(value.to_string()))
            .unwrap_or_else(Body::empty);
        let mut request = Request::builder()
            .uri(uri)
            .method(method)
            .body(body)
            .expect("build test request");

        if has_payload {
            request.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
        }
        match auth {
            TestAuth::Anonymous => {}
            TestAuth::Cookie(session) => {
                request.headers_mut().insert(
                    header::COOKIE,
                    HeaderValue::from_str(&session.cookie).expect("valid fixture cookie"),
                );
            }
            TestAuth::Bearer(session) => {
                request.headers_mut().insert(
                    header::AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", session.token))
                        .expect("valid bearer token"),
                );
            }
        }
        for (name, value) in extra_headers.iter() {
            request.headers_mut().insert(name.clone(), value.clone());
        }

        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("execute test request")
    }
}

pub async fn response_json(response: Response<Body>) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("response body must be JSON")
}
