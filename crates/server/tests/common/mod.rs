//! Integration test harness: starts a real Postgres 17 (testcontainers),
//! applies the migrations and builds the same `Router` as production.
//!
//! Requires Docker to be running.
#![allow(dead_code)]

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use jsonwebtoken::{EncodingKey, Header, get_current_timestamp};
use nelcota_api::{ApiSettings, CatalogHandle, spawn_reload_listener};
use nelcota_auth::{
    AuthSettings, AuthState, Email, Keys, MailError, Mailer, Passwords, RateLimiter,
    generate_ed25519_private_key,
};
use nelcota_core::db;
use nelcota_server::{AppState, app, load_catalog};
use serde_json::{Value, json};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use tower::ServiceExt;
use uuid::Uuid;

pub const JWT_SECRET: &str = "test-secret-with-more-than-32-characters";
pub const AUTHENTICATOR_PASSWORD: &str = "test-authenticator-password";
pub const ADMIN_EMAIL: &str = "admin@example.com";
pub const SSO_SECRET: &str = "host-shared-secret-with-32-chars+";

/// The host's project list used in the tests (this app is "shop").
fn registry_file() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nelcota-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("projects.json");
    std::fs::write(
        &path,
        json!({
            "panel_login": "shared",
            "projects": [
                { "name": "shop", "url": "https://shop.example.com" },
                { "name": "blog", "url": "https://blog.example.com" },
            ],
        })
        .to_string(),
    )
    .unwrap();
    path
}
pub const ADMIN_PASSWORD: &str = "test-admin-password";

/// App page that receives the recovery link in the tests.
pub const RECOVERY_URL: &str = "https://app.example.com/new-password";

/// Test mailer: keeps the messages instead of sending them.
#[derive(Default)]
pub struct Outbox {
    sent: std::sync::Mutex<Vec<Email>>,
}

impl Outbox {
    pub fn sent(&self) -> Vec<Email> {
        self.sent.lock().unwrap().clone()
    }

    /// Waits until there are `n` messages (sending runs in the background).
    pub async fn wait_for(&self, n: usize) -> Vec<Email> {
        for _ in 0..250 {
            let sent = self.sent();
            if sent.len() >= n {
                return sent;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("expected {n} email(s), got {}", self.sent().len());
    }
}

impl Mailer for Outbox {
    fn send(
        &self,
        email: Email,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), MailError>> + Send + '_>>
    {
        self.sent.lock().unwrap().push(email);
        Box::pin(async { Ok(()) })
    }
}

pub struct Options {
    pub pool_size: usize,
    /// Password recovery on (with the [`Outbox`] instead of SMTP).
    pub mail: bool,
    /// `migrations/` folder the panel sees.
    pub migrations_dir: Option<std::path::PathBuf>,
    pub rate_limit_per_minute: u32,
    pub access_ttl_secs: u64,
    pub max_rows: Option<i64>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            pool_size: 4,
            mail: true,
            migrations_dir: None,
            rate_limit_per_minute: 10_000,
            access_ttl_secs: 900,
            max_rows: None,
        }
    }
}

pub struct TestApp {
    pub router: Router,
    pub pool: deadpool_postgres::Pool,
    pub admin: tokio_postgres::Config,
    pub admin_client: tokio_postgres::Client,
    pub keys: Arc<Keys>,
    pub catalog: Arc<CatalogHandle>,
    pub user_a: Uuid,
    pub user_b: Uuid,
    pub outbox: Arc<Outbox>,
    _container: ContainerAsync<Postgres>,
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
    pub text: String,
}

impl TestApp {
    pub async fn spawn() -> Self {
        Self::spawn_with(Options::default()).await
    }

    pub async fn spawn_with(options: Options) -> Self {
        let container = Postgres::default()
            .with_tag("17-alpine")
            .with_cmd([
                "postgres",
                "-c",
                "fsync=off",
                "-c",
                "shared_preload_libraries=pg_stat_statements",
            ])
            .start()
            .await
            .expect("Docker must be running for the integration tests");
        let host = container.get_host().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let admin: tokio_postgres::Config =
            format!("postgres://postgres:postgres@{host}:{port}/postgres")
                .parse()
                .unwrap();

        db::bootstrap(&admin, AUTHENTICATOR_PASSWORD, 10)
            .await
            .unwrap();

        // Fixture: the example table + one row per user, inserted as the
        // superuser (which bypasses RLS).
        let (user_a, user_b) = (Uuid::new_v4(), Uuid::new_v4());
        let (admin_client, connection) = admin.connect(tokio_postgres::NoTls).await.unwrap();
        tokio::spawn(connection);
        admin_client
            .batch_execute(include_str!("../../../../examples/todos.sql"))
            .await
            .unwrap();
        admin_client
            .batch_execute(include_str!("../fixtures/api.sql"))
            .await
            .unwrap();
        admin_client
            .execute(
                "INSERT INTO public.todos (user_id, title) VALUES ($1, 'task of A'), ($2, 'task of B')",
                &[&user_a, &user_b],
            )
            .await
            .unwrap();

        // Signs with EdDSA (as in production) and also accepts HS256, so the
        // tests can mint arbitrary tokens with `token()`.
        let keys = Arc::new(
            Keys::new(
                Some(&generate_ed25519_private_key()),
                Some(JWT_SECRET.as_bytes()),
            )
            .unwrap(),
        );
        let pool = db::api_pool(&admin, AUTHENTICATOR_PASSWORD, options.pool_size);
        let catalog = load_catalog(&pool, "public").await.unwrap();
        spawn_reload_listener(
            catalog.clone(),
            pool.clone(),
            db::authenticator_config(&admin, AUTHENTICATOR_PASSWORD),
        );
        let state = AppState {
            pool: pool.clone(),
            verifier: keys.clone(),
            catalog: catalog.clone(),
            api: Arc::new(ApiSettings {
                max_rows: options.max_rows,
            }),
        };
        let panel = nelcota_admin::AdminState {
            db: db::admin_pool(&admin, 2),
            db_config: admin.clone(),
            catalog: catalog.clone(),
            credentials: Arc::new(nelcota_admin::Credentials {
                email: ADMIN_EMAIL.into(),
                password_hash: nelcota_auth::hash_password(ADMIN_PASSWORD).unwrap(),
            }),
            sessions: Arc::default(),
            limiter: Arc::new(RateLimiter::new(1000)),
            secure_cookies: false,
            host: Arc::new(nelcota_admin::HostLink {
                project: "shop".into(),
                registry: Some(registry_file()),
                sso: Some(nelcota_admin::Sso::new(SSO_SECRET.as_bytes())),
            }),
            tokens: Arc::new(nelcota_admin::TokenIssuer {
                keys: keys.clone(),
                issuer: "nelcota-test".into(),
            }),
            migrations_dir: options.migrations_dir.clone(),
        };
        let outbox = Arc::new(Outbox::default());
        let auth = AuthState {
            mailer: options.mail.then(|| outbox.clone() as Arc<dyn Mailer>),
            pool: pool.clone(),
            keys: keys.clone(),
            passwords: Arc::new(Passwords::new(2)),
            limiter: Arc::new(RateLimiter::new(options.rate_limit_per_minute)),
            settings: Arc::new(AuthSettings {
                issuer: "nelcota-test".into(),
                access_ttl_secs: options.access_ttl_secs,
                refresh_ttl_days: 30,
                signup_enabled: true,
                trust_proxy: false,
                recovery_url: options.mail.then(|| RECOVERY_URL.to_owned()),
            }),
        };
        TestApp {
            router: app(state, auth, Some(panel), Duration::from_secs(10)),
            pool,
            admin,
            admin_client,
            keys,
            catalog,
            user_a,
            user_b,
            outbox,
            _container: container,
        }
    }

    pub async fn request(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Reply {
        self.request_with(method, path, token, body, &[]).await
    }

    pub async fn request_with(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
        extra_headers: &[(&str, &str)],
    ) -> Reply {
        let mut request = Request::builder().method(method).uri(encode_uri(path));
        for (name, value) in extra_headers {
            request = request.header(*name, *value);
        }
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let body = match body {
            Some(json) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(json.to_string())
            }
            None => Body::empty(),
        };
        let response = self
            .router
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Reply {
            status,
            headers,
            body,
            text: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    /// Request with an arbitrary body (panel forms).
    pub async fn raw(
        &self,
        method: Method,
        path: &str,
        headers: &[(&str, &str)],
        body: String,
    ) -> Reply {
        let mut request = Request::builder().method(method).uri(encode_uri(path));
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = self
            .router
            .clone()
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            headers,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            text: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    pub async fn get(&self, path: &str, token: Option<&str>) -> (StatusCode, Value) {
        let reply = self.request(Method::GET, path, token, None).await;
        (reply.status, reply.body)
    }

    pub async fn post(&self, path: &str, token: Option<&str>, body: Value) -> Reply {
        self.request(Method::POST, path, token, Some(body)).await
    }
}

/// Percent-encodes the bytes that cannot appear raw in a URI (space, quotes,
/// accents...), so the tests can write readable queries.
pub fn encode_uri(path: &str) -> String {
    path.bytes()
        .map(|b| match b {
            b' ' | b'"' | b'\\' | b'<' | b'>' | b'`' | b'{' | b'}' | b'|' | b'^' | 0x80.. => {
                format!("%{b:02X}")
            }
            _ => (b as char).to_string(),
        })
        .collect()
}

/// HS256 JWT with the given claims (default `exp`: 1 hour from now).
pub fn token(claims: Value) -> String {
    token_with_secret(claims, JWT_SECRET)
}

pub fn token_with_secret(mut claims: Value, secret: &str) -> String {
    if claims.get("exp").is_none() {
        claims["exp"] = json!(get_current_timestamp() + 3600);
    }
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

pub fn user_token(sub: Uuid) -> String {
    token(json!({ "role": "authenticated", "sub": sub }))
}

pub fn service_token() -> String {
    token(json!({ "role": "service_role" }))
}
