//! Painel administrativo em `/admin`: tabelas, editor SQL, usuários e
//! policies RLS (com alerta de "tabela sem RLS").
//!
//! - Login próprio (`NELCOTA_ADMIN_EMAIL` + hash argon2id), separado dos
//!   usuários finais. Sessão em memória, cookie `HttpOnly; SameSite=Strict`.
//! - O painel fala com o banco pela conexão administrativa: o admin vê e altera
//!   tudo, como no `psql`. Por isso o login é obrigatório em todas as rotas.
//! - HTML montado no servidor; ~3 KB de JS e CSS embutidos (`rust-embed`), sem
//!   scripts inline (CSP `script-src 'self'`).

mod html;
mod pages;
mod sql;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Form, Router,
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use deadpool_postgres::Pool;
use nelcota_api::CatalogHandle;
use nelcota_auth::RateLimiter;
use rust_embed::RustEmbed;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const COOKIE: &str = "nelcota_admin";
const SESSION_TTL: Duration = Duration::from_secs(12 * 3600);

#[derive(RustEmbed)]
#[folder = "assets/"]
struct Assets;

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
        .route("/admin/", get(pages::dashboard))
        .route("/admin/tables/{name}", get(pages::table))
        .route("/admin/tables/{name}/new", get(pages::new_row))
        .route("/admin/tables/{name}/insert", post(pages::insert_row))
        .route("/admin/tables/{name}/edit", get(pages::edit_row))
        .route("/admin/tables/{name}/update", post(pages::update_row))
        .route("/admin/tables/{name}/delete", post(pages::delete_row))
        .route("/admin/sql", get(sql::editor).post(sql::run))
        .route("/admin/users", get(pages::users))
        .route("/admin/users/{id}/revoke", post(pages::revoke_sessions))
        .route("/admin/users/{id}/delete", post(pages::delete_user))
        .route("/admin/policies", get(pages::policies))
        .route("/admin/logout", post(logout))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ));

    Router::new()
        .route("/admin", get(|| async { Redirect::to("/admin/") }))
        .route("/admin/login", get(login_page).post(login))
        .route("/admin/assets/{file}", get(asset))
        .merge(protected)
        .layer(middleware::from_fn(same_origin))
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
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
        return next.run(request).await;
    }
    if request.method() == Method::GET {
        Redirect::to("/admin/login").into_response()
    } else {
        (StatusCode::UNAUTHORIZED, "sessão expirada: entre de novo").into_response()
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
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
             frame-ancestors 'none'; form-action 'self'; base-uri 'none'",
        ),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    // `same-origin`, e não `no-referrer`: com `no-referrer` o navegador manda
    // `Origin: null` nos POSTs de formulário e a checagem de CSRF recusaria o
    // próprio painel. `same-origin` também não vaza a URL para outros sites.
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn asset(Path(file): Path<String>) -> Response {
    let Some(content) = Assets::get(&file) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = match file.rsplit('.').next() {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    };
    (
        [(header::CONTENT_TYPE, content_type)],
        Body::from(content.data.into_owned()),
    )
        .into_response()
}

async fn login_page() -> Response {
    html::login(None).into_response()
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

async fn login(State(state): State<AdminState>, Form(form): Form<LoginForm>) -> Response {
    if state.limiter.check("admin-login").is_err() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            html::login(Some("Muitas tentativas. Aguarde um minuto.")),
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
        return (
            StatusCode::UNAUTHORIZED,
            html::login(Some("Email ou senha inválidos.")),
        )
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
        StatusCode::SEE_OTHER,
        [
            (header::SET_COOKIE, cookie),
            (header::LOCATION, "/admin/".to_owned()),
        ],
    )
        .into_response()
}

async fn logout(State(state): State<AdminState>, headers: HeaderMap) -> Response {
    if let Some(token) = session_token(&headers) {
        state.sessions.remove(&token);
    }
    (
        StatusCode::SEE_OTHER,
        [
            (
                header::SET_COOKIE,
                format!("{COOKIE}=; Path=/admin; HttpOnly; SameSite=Strict; Max-Age=0"),
            ),
            (header::LOCATION, "/admin/login".to_owned()),
        ],
    )
        .into_response()
}
