//! Sessions, email links and native OAuth/PKCE. Session clones share one lifecycle lock.
use crate::{Client, Error, Result, TokenMode, http::Spec};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, broadcast};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: String,
    pub email: String,
    pub email_confirmed_at: Option<String>,
    pub user_metadata: BTreeMap<String, Value>,
    pub created_at: String,
    pub last_sign_in_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub expires_at: u64,
    pub refresh_token: String,
    pub user: User,
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("user", &self.user)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}
impl Session {
    pub fn expires_in_now(&self) -> Duration {
        Duration::from_secs(self.expires_at.saturating_sub(now()))
    }
    fn validate(&self) -> Result<()> {
        if self.access_token.is_empty()
            || self.refresh_token.is_empty()
            || self.token_type != "bearer"
            || self.user.id.is_empty()
        {
            return Err(Error::InvalidResponse {
                status: 200,
                message: "invalid session".into(),
            });
        }
        Ok(())
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Persist a session in a cookie, keychain or another caller-owned store.
/// Share a Client by cloning it. Independent clients/processes using the same store
/// must coordinate rotation externally; the storage interface alone is not a lock.
pub trait SessionStorage: Send + Sync {
    fn load(&self) -> StorageFuture<'_, Option<Session>>;
    fn save<'a>(&'a self, session: Option<&'a Session>) -> StorageFuture<'a, ()>;
}

#[derive(Default)]
pub struct MemoryStorage(std::sync::Mutex<Option<Session>>);
impl SessionStorage for MemoryStorage {
    fn load(&self) -> StorageFuture<'_, Option<Session>> {
        Box::pin(async {
            self.0
                .lock()
                .map(|s| s.clone())
                .map_err(|_| Error::SessionStorage("poisoned memory store".into()))
        })
    }
    fn save<'a>(&'a self, session: Option<&'a Session>) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            *self
                .0
                .lock()
                .map_err(|_| Error::SessionStorage("poisoned memory store".into()))? =
                session.cloned();
            Ok(())
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthEventKind {
    SignedIn,
    SignedOut,
    Refreshed,
    UserUpdated,
}
#[derive(Clone, Debug)]
pub struct AuthEvent {
    pub kind: AuthEventKind,
    pub session: Option<Session>,
}

#[derive(Default)]
struct State {
    loaded: bool,
    session: Option<Session>,
    last_refresh: Option<Result<Session>>,
}
pub(crate) struct Manager {
    gate: Mutex<State>,
    epoch: AtomicU64,
    storage: Arc<dyn SessionStorage>,
    margin: Duration,
    events: broadcast::Sender<AuthEvent>,
}
impl Manager {
    pub(crate) fn new(storage: Arc<dyn SessionStorage>, margin: Duration) -> Self {
        Self {
            gate: Mutex::new(State::default()),
            epoch: AtomicU64::new(0),
            storage,
            margin,
            events: broadcast::channel(32).0,
        }
    }
    async fn load(&self, state: &mut State) -> Result<()> {
        if !state.loaded {
            let session = self.storage.load().await?;
            if let Some(session) = &session {
                session.validate()?;
            }
            state.session = session;
            state.loaded = true;
        }
        Ok(())
    }
    async fn save(
        &self,
        state: &mut State,
        session: Option<Session>,
        event: AuthEventKind,
    ) -> Result<()> {
        // Keep a rotated token even if external persistence fails. Never reuse its predecessor.
        state.session = session;
        state.loaded = true;
        state.last_refresh = None;
        self.epoch.fetch_add(1, Ordering::Release);
        let result = self.storage.save(state.session.as_ref()).await;
        let _ = self.events.send(AuthEvent {
            kind: event,
            session: state.session.clone(),
        });
        result
    }
}

#[derive(Clone, Debug)]
pub struct AuthClient {
    pub(crate) client: Client,
}

#[derive(Debug)]
pub struct SignUp {
    pub user: User,
    pub session: Option<Session>,
}

impl AuthClient {
    pub fn subscribe(&self) -> broadcast::Receiver<AuthEvent> {
        self.client.inner.auth.events.subscribe()
    }

    /// Current session, refreshed on demand. Explicit tokens do not change this session.
    pub async fn get_session(&self) -> Result<Option<Session>> {
        let manager = &self.client.inner.auth;
        let observed = manager.epoch.load(Ordering::Acquire);
        let mut state = manager.gate.lock().await;
        manager.load(&mut state).await?;
        match &state.session {
            None => Ok(None),
            Some(session) if session.expires_in_now() > manager.margin => Ok(Some(session.clone())),
            _ => {
                drop(state);
                self.refresh_since(observed).await.map(Some)
            }
        }
    }

    pub(crate) fn access_token(&self) -> StorageFuture<'_, Option<String>> {
        Box::pin(async move {
            match self.get_session().await {
                Ok(session) => Ok(session.map(|s| s.access_token)),
                Err(error) => {
                    // A transient refresh failure still sends the previous identity, never anon.
                    let state = self.client.inner.auth.gate.lock().await;
                    if error.code() != "invalid_grant"
                        && let Some(session) = &state.session
                    {
                        return Ok(Some(session.access_token.clone()));
                    }
                    Err(error)
                }
            }
        })
    }

    pub async fn refresh_session(&self) -> Result<Session> {
        self.refresh_since(self.client.inner.auth.epoch.load(Ordering::Acquire))
            .await
    }

    async fn refresh_since(&self, observed: u64) -> Result<Session> {
        if self
            .client
            .options
            .cancellation
            .as_ref()
            .is_some_and(|t| t.is_cancelled())
        {
            return Err(Error::Cancelled);
        }
        let mut auth = self.clone();
        // Rotation continues if a waiter is dropped/cancelled after the server consumes the token.
        auth.client.options.cancellation = None;
        let task = tokio::spawn(async move {
            let manager = &auth.client.inner.auth;
            let mut state = manager.gate.lock().await;
            manager.load(&mut state).await?;
            if manager.epoch.load(Ordering::Acquire) != observed {
                if let Some(result) = &state.last_refresh {
                    return result.clone();
                }
                return state.session.clone().ok_or(Error::SessionMissing);
            }
            let current = state.session.as_ref().ok_or(Error::SessionMissing)?;
            let result = auth
                .token(
                    "refresh_token",
                    json!({"refresh_token": current.refresh_token}),
                )
                .await;
            let result = match result {
                Ok(session) => manager
                    .save(&mut state, Some(session.clone()), AuthEventKind::Refreshed)
                    .await
                    .map(|()| session),
                Err(error) => {
                    if error.code() == "invalid_grant" {
                        let _ = manager
                            .save(&mut state, None, AuthEventKind::SignedOut)
                            .await;
                    }
                    Err(error)
                }
            };
            manager.epoch.fetch_add(1, Ordering::Release);
            state.last_refresh = Some(result.clone());
            result
        });
        let wait = async {
            task.await
                .map_err(|_| Error::Network("refresh task failed".into()))?
        };
        if let Some(token) = &self.client.options.cancellation {
            tokio::select! { biased; _ = token.cancelled() => Err(Error::Cancelled), result = wait => result }
        } else {
            wait.await
        }
    }

    pub async fn set_session(&self, session: Session) -> Result<()> {
        session.validate()?;
        let manager = &self.client.inner.auth;
        let mut state = manager.gate.lock().await;
        manager
            .save(&mut state, Some(session), AuthEventKind::SignedIn)
            .await
    }

    async fn token(&self, grant: &str, body: Value) -> Result<Session> {
        let mut spec = Spec::new(Method::POST, "/auth/v1/token")
            .json(body)
            .unauthenticated();
        spec.params.push(("grant_type".into(), grant.into()));
        let response = self.client.json::<Session>(spec).await?;
        response.data.validate()?;
        Ok(response.data)
    }

    pub async fn sign_up(
        &self,
        email: &str,
        password: &str,
        metadata: Option<Value>,
    ) -> Result<SignUp> {
        let mut body = json!({"email": address(email)?, "password": password});
        if let Some(data) = metadata {
            if !data.is_object() {
                return Err(Error::Usage("signup metadata must be an object".into()));
            }
            body["data"] = data;
        }
        let manager = &self.client.inner.auth;
        let mut state = manager.gate.lock().await;
        let response = self
            .client
            .json::<Value>(
                Spec::new(Method::POST, "/auth/v1/signup")
                    .json(body)
                    .unauthenticated(),
            )
            .await?;
        if response.data.get("access_token").is_some() {
            let session: Session = decode(response.data, response.status)?;
            session.validate()?;
            manager
                .save(&mut state, Some(session.clone()), AuthEventKind::SignedIn)
                .await?;
            Ok(SignUp {
                user: session.user.clone(),
                session: Some(session),
            })
        } else {
            Ok(SignUp {
                user: decode(response.data["user"].clone(), response.status)?,
                session: None,
            })
        }
    }

    pub async fn sign_in_with_password(&self, email: &str, password: &str) -> Result<Session> {
        let body = json!({"email": address(email)?, "password": password});
        let manager = &self.client.inner.auth;
        let mut state = manager.gate.lock().await;
        let session = self.token("password", body).await?;
        manager
            .save(&mut state, Some(session.clone()), AuthEventKind::SignedIn)
            .await?;
        Ok(session)
    }

    /// Read the server's current user. Scoped clients use their explicit caller token.
    pub async fn get_user(&self) -> Result<User> {
        let token = match &self.client.token {
            TokenMode::Fixed(Some(token)) => token.clone(),
            TokenMode::Fixed(None) => return Err(Error::SessionMissing),
            TokenMode::Session => {
                self.get_session()
                    .await?
                    .ok_or(Error::SessionMissing)?
                    .access_token
            }
        };
        let user = self
            .client
            .json::<User>(Spec::new(Method::GET, "/auth/v1/user").token(&token))
            .await?
            .data;
        let manager = &self.client.inner.auth;
        let mut state = manager.gate.lock().await;
        if let Some(session) = state
            .session
            .as_ref()
            .filter(|s| s.access_token == token && s.user != user)
        {
            let mut session = session.clone();
            session.user = user.clone();
            manager
                .save(&mut state, Some(session), AuthEventKind::UserUpdated)
                .await?;
        }
        Ok(user)
    }

    /// Serialize against refresh and clear locally even when server revocation fails.
    pub async fn sign_out(&self) -> Result<()> {
        let manager = &self.client.inner.auth;
        let mut state = manager.gate.lock().await;
        manager.load(&mut state).await?;
        let result = match &state.session {
            Some(session) => self
                .client
                .bytes(Spec::new(Method::POST, "/auth/v1/logout").token(&session.access_token))
                .await
                .map(|_| ()),
            None => Ok(()),
        };
        let cleared = manager
            .save(&mut state, None, AuthEventKind::SignedOut)
            .await;
        result.and(cleared)
    }

    pub async fn request_password_reset(&self, email: &str) -> Result<()> {
        self.email("recover", json!({"email": address(email)?}))
            .await
    }
    pub async fn send_magic_link(&self, email: &str) -> Result<()> {
        self.email("magiclink", json!({"email": address(email)?}))
            .await
    }
    pub async fn resend_confirmation(&self, email: &str) -> Result<()> {
        self.email("resend", json!({"email": address(email)?, "type":"signup"}))
            .await
    }
    async fn email(&self, endpoint: &str, body: Value) -> Result<()> {
        self.client
            .bytes(
                Spec::new(Method::POST, format!("/auth/v1/{endpoint}"))
                    .json(body)
                    .unauthenticated(),
            )
            .await
            .map(|_| ())
    }
    pub async fn verify_email_link(&self, link: &EmailLink) -> Result<Session> {
        if link.kind == EmailLinkType::Recovery {
            return Err(Error::Usage("recovery links require reset_password".into()));
        }
        self.verify(json!({"type": link.kind.as_str(), "token": link.token}))
            .await
    }
    pub async fn reset_password(&self, token: &str, password: &str) -> Result<Session> {
        self.verify(json!({"type":"recovery", "token": token, "password": password}))
            .await
    }
    async fn verify(&self, body: Value) -> Result<Session> {
        let manager = &self.client.inner.auth;
        let mut state = manager.gate.lock().await;
        let session = self
            .client
            .json::<Session>(
                Spec::new(Method::POST, "/auth/v1/verify")
                    .json(body)
                    .unauthenticated(),
            )
            .await?
            .data;
        session.validate()?;
        manager
            .save(&mut state, Some(session.clone()), AuthEventKind::SignedIn)
            .await?;
        Ok(session)
    }

    /// Return the authorization URL and verifier. The application opens the browser
    /// and keeps this flow until its callback; the SDK does not navigate or bind a port.
    pub fn begin_oauth(&self, provider: OAuthProvider, redirect_to: &str) -> Result<PkceFlow> {
        let redirect = Url::parse(redirect_to)
            .map_err(|_| Error::Usage("redirect_to must be an absolute URL".into()))?;
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes)
            .map_err(|_| Error::Usage("secure random generator unavailable".into()))?;
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut url = self.client.url("/auth/v1/authorize")?;
        url.query_pairs_mut().extend_pairs([
            ("provider", provider.as_str()),
            ("redirect_to", redirect.as_str()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
        ]);
        Ok(PkceFlow { url, verifier })
    }
    pub async fn exchange_code(&self, code: &str, verifier: &str) -> Result<Session> {
        if !(43..=128).contains(&verifier.len())
            || !verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
        {
            return Err(Error::Usage("invalid PKCE verifier".into()));
        }
        let manager = &self.client.inner.auth;
        let mut state = manager.gate.lock().await;
        let session = self
            .token("pkce", json!({"auth_code":code, "code_verifier":verifier}))
            .await?;
        manager
            .save(&mut state, Some(session.clone()), AuthEventKind::SignedIn)
            .await?;
        Ok(session)
    }
    /// Remove OAuth callback credentials from a URL before handing it to application logs/history.
    pub async fn handle_redirect(&self, url: &mut Url, flow: PkceFlow) -> Result<Option<Session>> {
        let code = url
            .query_pairs()
            .find(|(k, _)| k == "code")
            .map(|(_, v)| v.into_owned());
        let error = url
            .query_pairs()
            .find(|(k, _)| k == "error")
            .map(|(_, v)| v.into_owned());
        let pairs: Vec<_> = url
            .query_pairs()
            .filter(|(k, _)| k != "code" && k != "error")
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        url.set_query(None);
        if !pairs.is_empty() {
            url.query_pairs_mut().extend_pairs(pairs);
        }
        if let Some(code) = error {
            return Err(Error::Http {
                status: 400,
                code,
                message: "OAuth sign-in failed".into(),
                retry_after: None,
                db: None,
            });
        }
        match code {
            Some(code) => self.exchange_code(&code, &flow.verifier).await.map(Some),
            None => Ok(None),
        }
    }

    /// Opt-in background refresh. Drop/stop the handle to stop the timer.
    pub fn start_auto_refresh(&self) -> AutoRefresh {
        let weak = Arc::downgrade(&self.client.inner);
        let mut events = self.subscribe();
        let handle = tokio::spawn(async move {
            loop {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                let client = Client {
                    inner,
                    token: TokenMode::Session,
                    options: Default::default(),
                };
                let manager = &client.inner.auth;
                let mut state = manager.gate.lock().await;
                if manager.load(&mut state).await.is_err() {
                    drop(state);
                    drop(client);
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
                let session = state.session.clone();
                drop(state);
                let delay = session
                    .map(|s| s.expires_in_now().saturating_sub(client.inner.auth.margin))
                    .unwrap_or(Duration::from_secs(30));
                drop(client);
                tokio::select! {
                    _ = tokio::time::sleep(delay.max(Duration::from_secs(1))) => {
                        let Some(inner) = weak.upgrade() else { return; };
                        let client = Client { inner, token: TokenMode::Session, options: Default::default() };
                        if client.auth().get_session().await.is_err() { tokio::time::sleep(Duration::from_secs(10)).await; }
                    }
                    result = events.recv() => { if matches!(result, Err(broadcast::error::RecvError::Closed)) { return; } }
                }
            }
        });
        AutoRefresh(handle)
    }
}

pub struct AutoRefresh(tokio::task::JoinHandle<()>);
impl AutoRefresh {
    pub fn stop(self) {
        self.0.abort();
    }
}
impl Drop for AutoRefresh {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[derive(Clone, Copy, Debug)]
pub enum OAuthProvider {
    Google,
    Github,
}
impl OAuthProvider {
    fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Github => "github",
        }
    }
}

pub struct PkceFlow {
    pub url: Url,
    pub verifier: String,
}
impl std::fmt::Debug for PkceFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PkceFlow")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmailLinkType {
    Signup,
    MagicLink,
    Recovery,
}
impl EmailLinkType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Signup => "signup",
            Self::MagicLink => "magiclink",
            Self::Recovery => "recovery",
        }
    }
}
pub struct EmailLink {
    pub kind: EmailLinkType,
    pub token: String,
}
impl std::fmt::Debug for EmailLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailLink")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
impl EmailLink {
    pub fn parse(url: &mut Url) -> Option<Self> {
        let fragment = url.fragment()?;
        let pairs: BTreeMap<_, _> = url::form_urlencoded::parse(fragment.as_bytes())
            .into_owned()
            .collect();
        let kind = match pairs.get("type")?.as_str() {
            "signup" => EmailLinkType::Signup,
            "magiclink" => EmailLinkType::MagicLink,
            "recovery" => EmailLinkType::Recovery,
            _ => return None,
        };
        let token = pairs.get("token")?.clone();
        if token.is_empty() || token.len() > 128 {
            return None;
        }
        url.set_fragment(None);
        Some(Self { kind, token })
    }
}

fn address(email: &str) -> Result<&str> {
    let email = email.trim();
    if email.is_empty() {
        Err(Error::Usage("email is required".into()))
    } else {
        Ok(email)
    }
}
fn decode<T: serde::de::DeserializeOwned>(value: Value, status: u16) -> Result<T> {
    serde_json::from_value(value).map_err(|e| Error::InvalidResponse {
        status,
        message: e.to_string(),
    })
}
