//! Harness dos testes de integração: sobe um Postgres 17 real (testcontainers),
//! aplica as migrações e monta o mesmo `Router` de produção.
//!
//! Requer Docker em execução.
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
    AuthSettings, AuthState, Keys, Passwords, RateLimiter, generate_ed25519_private_key,
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

pub const JWT_SECRET: &str = "segredo-de-teste-com-mais-de-32-caracteres";
pub const AUTHENTICATOR_PASSWORD: &str = "senha-do-authenticator-de-teste";

pub struct Options {
    pub pool_size: usize,
    pub rate_limit_per_minute: u32,
    pub access_ttl_secs: u64,
    pub max_rows: Option<i64>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            pool_size: 4,
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
    _container: ContainerAsync<Postgres>,
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
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
            .expect("Docker precisa estar rodando para os testes de integração");
        let host = container.get_host().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let admin: tokio_postgres::Config =
            format!("postgres://postgres:postgres@{host}:{port}/postgres")
                .parse()
                .unwrap();

        db::bootstrap(&admin, AUTHENTICATOR_PASSWORD, 10)
            .await
            .unwrap();

        // Fixture: tabela de exemplo + uma linha para cada usuário, inseridas
        // como superusuário (que ignora RLS).
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
                "INSERT INTO public.todos (user_id, title) VALUES ($1, 'tarefa de A'), ($2, 'tarefa de B')",
                &[&user_a, &user_b],
            )
            .await
            .unwrap();

        // Assina com EdDSA (como em produção) e também aceita HS256, para os
        // testes cunharem tokens arbitrários com `token()`.
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
        let auth = AuthState {
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
            }),
        };
        TestApp {
            router: app(state, auth, Duration::from_secs(10)),
            pool,
            admin,
            admin_client,
            keys,
            catalog,
            user_a,
            user_b,
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

/// Percent-encode dos bytes que não podem aparecer crus numa URI (espaço,
/// aspas, acentos...), para os testes escreverem queries legíveis.
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

/// JWT HS256 com as claims dadas (`exp` padrão: daqui a 1 h).
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
