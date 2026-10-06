//! Painel administrativo em `/admin`.
//!
//! - Frontend: SPA em Svelte 5 (`crates/admin/ui`), compilada com Vite para
//!   `ui/dist` e embutida no binário (`rust-embed`). Sem Node em produção.
//! - Backend: API JSON em `/admin/api/*`, protegida por login próprio
//!   (`NELCOTA_ADMIN_EMAIL` + hash argon2id), separado dos usuários finais.
//!   Sessão em memória, cookie `HttpOnly; SameSite=Strict`, checagem de origem
//!   em todo request que muda estado (CSRF).
//! - O painel fala com o banco pela conexão administrativa: o admin vê e
//!   altera tudo, como no `psql`. Escritas em tabelas reutilizam o construtor
//!   de SQL da API (identificadores só do catálogo, valores parametrizados).

mod api;
mod sql;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use deadpool_postgres::Pool;
use nelcota_api::CatalogHandle;
use nelcota_auth::RateLimiter;
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

const COOKIE: &str = "nelcota_admin";
const SESSION_TTL: Duration = Duration::from_secs(12 * 3600);

/// Build do frontend (`npm run build` em `crates/admin/ui`).
#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Ui;

pub struct Credentials {
    pub email: String,
    pub password_hash: String,
}

/// Sessões do painel em memória (chave = SHA-256 do token do cookie).
#[derive(Default)]
pub struct Sessions {
    active: Mutex<HashMap<Vec<u8>, Instant>>,
}

impl Sessions {
    fn create(&self) -> String {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("fonte de aleatoriedade do sistema indisponível");
        let token = URL_SAFE_NO_PAD.encode(bytes);
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        active.retain(|_, expires| *expires > now);
        active.insert(Sha256::digest(token.as_bytes()).to_vec(), now + SESSION_TTL);
        token
    }

    fn valid(&self, token: &str) -> bool {
        let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        active
            .get(&Sha256::digest(token.as_bytes())[..])
            .is_some_and(|expires| *expires > Instant::now())
    }

    fn remove(&self, token: &str) {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        active.remove(&Sha256::digest(token.as_bytes())[..]);
    }
}

#[derive(Clone)]
pub struct AdminState {
    /// Pool administrativo (role dona do schema).
    pub db: Pool,
    /// Config administrativa, para conexões dedicadas do editor SQL.
    pub db_config: tokio_postgres::Config,
    pub catalog: Arc<CatalogHandle>,
    pub credentials: Arc<Credentials>,
    pub sessions: Arc<Sessions>,
    pub limiter: Arc<RateLimiter>,
    /// Cookie com `Secure` (atrás do Caddy/HTTPS).
    pub secure_cookies: bool,
}

pub fn router(state: AdminState) -> Router {
    let protected = Router::new()
        .route("/admin/api/session", get(session))
        .route("/admin/api/logout", post(logout))
        .route("/admin/api/overview", get(api::overview))
        .route("/admin/api/schema", get(api::schema))
        .route("/admin/api/tables", get(api::tables))
        .route("/admin/api/tables/{name}", get(api::table))
        .route(
            "/admin/api/tables/{name}/rows",
            post(api::insert_row)
                .patch(api::update_row)
                .delete(api::delete_rows),
        )
        .route("/admin/api/sql", post(sql::run))
        .route("/admin/api/users", get(api::users))
        .route("/admin/api/users/{id}/revoke", post(api::revoke_sessions))
        .route("/admin/api/users/{id}", delete(api::delete_user))
        .route("/admin/api/policies", get(api::policies))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ));

    Router::new()
        .route("/admin/api/login", post(login))
        .merge(protected)
        .route("/admin/api/{*rest}", get(api_not_found).post(api_not_found))
        .route("/admin/assets/{*file}", get(asset))
        .route(
            "/admin/favicon.svg",
            get(|| async { ui_file("favicon.svg") }),
        )
        .route("/admin", get(|| async { Redirect::to("/admin/") }))
        .route("/admin/", get(spa))
        .route("/admin/{*route}", get(spa))
        .layer(middleware::from_fn(same_origin))
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

/// Erro da API do painel: `{"error": "..."}`.
pub struct ApiError(StatusCode, String);

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        ApiError(StatusCode::BAD_REQUEST, message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        ApiError(StatusCode::NOT_FOUND, message.into())
    }
}

/// Qualquer outro erro (banco, pool, JSON) vira 500 com a mensagem: o admin é
/// o dono do banco e precisa do texto para agir.
impl<E: std::fmt::Display> From<E> for ApiError {
    fn from(err: E) -> Self {
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

async fn api_not_found() -> ApiError {
    ApiError::not_found("rota não encontrada")
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_owned())
}

async fn require_session(
    State(state): State<AdminState>,
    request: Request,
    next: Next,
) -> Response {
    let ok = session_token(request.headers()).is_some_and(|t| state.sessions.valid(&t));
    if ok {
        next.run(request).await
    } else {
        ApiError(
            StatusCode::UNAUTHORIZED,
            "sessão expirada: entre de novo".into(),
        )
        .into_response()
    }
}

/// Defesa contra CSRF além do `SameSite=Strict`: requests que mudam estado
/// precisam vir da mesma origem (navegadores sempre mandam `Origin` em POST).
async fn same_origin(request: Request, next: Next) -> Response {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        let headers = request.headers();
        let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
        if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            let matches = host.is_some_and(|host| {
                origin == format!("https://{host}") || origin == format!("http://{host}")
            });
            if !matches {
                return (StatusCode::FORBIDDEN, "origem não permitida").into_response();
            }
        }
        if headers
            .get("sec-fetch-site")
            .is_some_and(|v| v == "cross-site")
        {
            return (StatusCode::FORBIDDEN, "origem não permitida").into_response();
        }
    }
    next.run(request).await
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    // Scripts só do próprio servidor. Estilos inline são permitidos porque
    // as transições do Svelte, o CodeMirror e o posicionamento de menus
    // (floating-ui) injetam estilos em tempo de execução.
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
             img-src 'self' data:; font-src 'self'; connect-src 'self'; \
             frame-ancestors 'none'; form-action 'self'; base-uri 'none'",
        ),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    // `same-origin`, e não `no-referrer`: com `no-referrer` o navegador manda
    // `Origin: null` nos POSTs e a checagem de CSRF recusaria o próprio painel.
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    response
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

fn ui_file(path: &str) -> Response {
    let Some(file) = Ui::get(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    (
        [(header::CONTENT_TYPE, content_type(path))],
        Body::from(file.data.into_owned()),
    )
        .into_response()
}

/// Arquivos com hash no nome (gerados pelo Vite): cache longo e imutável.
async fn asset(Path(file): Path<String>) -> Response {
    let mut response = ui_file(&format!("assets/{file}"));
    if response.status().is_success() {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    response
}

/// Qualquer rota do painel devolve o `index.html` da SPA (roteamento no
/// cliente). A página em si não tem dados: tudo vem da API autenticada.
async fn spa() -> Response {
    let mut response = ui_file("index.html");
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

async fn login(State(state): State<AdminState>, Json(form): Json<LoginRequest>) -> Response {
    if state.limiter.check("admin-login").is_err() {
        return ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "Muitas tentativas. Aguarde um minuto.".into(),
        )
        .into_response();
    }
    // A senha é sempre verificada (mesmo com email errado), para o tempo de
    // resposta não revelar qual dos dois falhou.
    let hash = state.credentials.password_hash.clone();
    let password = form.password;
    let password_ok =
        tokio::task::spawn_blocking(move || nelcota_auth::verify_password(&password, &hash))
            .await
            .unwrap_or(false);
    let email_ok = form
        .email
        .trim()
        .eq_ignore_ascii_case(&state.credentials.email);
    if !(password_ok && email_ok) {
        tracing::warn!("login do painel recusado");
        return ApiError(StatusCode::UNAUTHORIZED, "Email ou senha inválidos.".into())
            .into_response();
    }
    let token = state.sessions.create();
    let secure = if state.secure_cookies { "; Secure" } else { "" };
    let cookie = format!(
        "{COOKIE}={token}; Path=/admin; HttpOnly; SameSite=Strict; Max-Age={}{secure}",
        SESSION_TTL.as_secs()
    );
    tracing::info!("login no painel");
    (
        [(header::SET_COOKIE, cookie)],
        Json(json!({ "email": state.credentials.email })),
    )
        .into_response()
}

async fn session(State(state): State<AdminState>) -> Json<serde_json::Value> {
    Json(json!({ "email": state.credentials.email }))
}

async fn logout(State(state): State<AdminState>, headers: HeaderMap) -> Response {
    if let Some(token) = session_token(&headers) {
        state.sessions.remove(&token);
    }
    (
        [(
            header::SET_COOKIE,
            format!("{COOKIE}=; Path=/admin; HttpOnly; SameSite=Strict; Max-Age=0"),
        )],
        Json(json!({ "ok": true })),
    )
        .into_response()
}
