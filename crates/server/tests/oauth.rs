//! Sign-in through an external provider against a real Postgres, with a local
//! fake of GitHub's endpoints: PKCE on both legs, the redirect allowlist,
//! account linking and takeover rules, and single-use codes.

mod common;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Form, Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use common::*;
use nelcota_auth::{OAuth, Provider, ProviderEndpoints};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const APP: &str = "https://app.example.com/auth";
const AUTHORIZE: &str = "https://github.example/login/oauth/authorize";
const VERIFIER: &str = "app-verifier-0123456789-abcdefghijklmnopqrstuvwxyz";

/// What the fake provider answers and what it was sent.
#[derive(Default)]
struct Fake {
    profile: Value,
    emails: Value,
    token_requests: Vec<HashMap<String, String>>,
    /// How long the code exchange takes (to overlap concurrent callbacks).
    token_delay: std::time::Duration,
}

type Shared = Arc<Mutex<Fake>>;

async fn token(
    State(fake): State<Shared>,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let good = form.get("code").map(String::as_str) == Some("good-code");
    let delay = fake.lock().unwrap().token_delay;
    tokio::time::sleep(delay).await;
    fake.lock().unwrap().token_requests.push(form);
    if good {
        (
            StatusCode::OK,
            Json(json!({ "access_token": "provider-token" })),
        )
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "bad_verification_code" })),
        )
    }
}

fn authorized(headers: &HeaderMap) -> bool {
    headers.get("authorization").and_then(|v| v.to_str().ok()) == Some("Bearer provider-token")
}

async fn user(State(fake): State<Shared>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(json!({})));
    }
    (StatusCode::OK, Json(fake.lock().unwrap().profile.clone()))
}

async fn emails(State(fake): State<Shared>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(json!([])));
    }
    (StatusCode::OK, Json(fake.lock().unwrap().emails.clone()))
}

struct Harness {
    app: TestApp,
    fake: Shared,
}

impl Harness {
    async fn spawn() -> Self {
        Self::spawn_with(Options::default()).await
    }

    async fn spawn_with(options: Options) -> Self {
        let fake: Shared = Arc::default();
        let router = Router::new()
            .route("/token", post(token))
            .route("/user", get(user))
            .route("/emails", get(emails))
            .with_state(fake.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

        let provider =
            Provider::github("client-1", "client-secret-1").with_endpoints(ProviderEndpoints {
                authorize: AUTHORIZE.into(),
                token: format!("{base}/token"),
                profile: format!("{base}/user"),
                emails: Some(format!("{base}/emails")),
            });
        let oauth = OAuth::new("https://api.example.com/", [APP], vec![provider]).unwrap();
        let app = TestApp::spawn_with(Options {
            oauth: Some(Arc::new(oauth)),
            ..options
        })
        .await;
        Harness { app, fake }
    }

    /// The account the provider reports for the next sign-in.
    fn account(&self, id: u64, email: Option<&str>, verified: bool) {
        let mut fake = self.fake.lock().unwrap();
        fake.profile = json!({ "id": id, "login": format!("user{id}"), "name": "Octo Cat", "avatar_url": "https://avatars.example/1" });
        fake.emails = match email {
            Some(email) => json!([{ "email": email, "primary": true, "verified": verified }]),
            None => json!([]),
        };
    }

    /// `/authorize` with the app's challenge; returns the provider URL.
    async fn authorize(&self, redirect_to: &str) -> Reply {
        let query = form_urlencoded::Serializer::new(String::new())
            .append_pair("provider", "github")
            .append_pair("redirect_to", redirect_to)
            .append_pair("code_challenge", &challenge(VERIFIER))
            .append_pair("code_challenge_method", "S256")
            .finish();
        self.app
            .raw(
                axum::http::Method::GET,
                &format!("/auth/v1/authorize?{query}"),
                &[],
                String::new(),
            )
            .await
    }

    async fn callback(&self, query: &str) -> Reply {
        self.app
            .raw(
                axum::http::Method::GET,
                &format!("/auth/v1/callback?{query}"),
                &[],
                String::new(),
            )
            .await
    }

    /// Up to the provider's callback; returns where the app was sent.
    async fn sign_in(&self) -> HashMap<String, String> {
        let reply = self.authorize(APP).await;
        assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.text);
        let state = &query_of(&location(&reply))["state"];
        let reply = self
            .callback(&format!("code=good-code&state={state}"))
            .await;
        assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.text);
        let back = location(&reply);
        assert!(back.starts_with(&format!("{APP}?")), "{back}");
        query_of(&back)
    }

    async fn redeem(&self, code: &str, verifier: &str) -> Reply {
        self.app
            .post(
                "/auth/v1/token?grant_type=pkce",
                None,
                json!({ "auth_code": code, "code_verifier": verifier }),
            )
            .await
    }

    /// A full sign-in that must succeed; returns the session.
    async fn session(&self) -> Value {
        let back = self.sign_in().await;
        let code = back
            .get("code")
            .unwrap_or_else(|| panic!("no code: {back:?}"));
        let reply = self.redeem(code, VERIFIER).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
        reply.body
    }
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn location(reply: &Reply) -> String {
    reply.headers["location"].to_str().unwrap().to_owned()
}

fn query_of(url: &str) -> HashMap<String, String> {
    let query = url.split_once('?').map(|(_, q)| q).unwrap_or_default();
    form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect()
}

async fn password_login(app: &TestApp, email: &str, password: &str) -> Reply {
    app.post(
        "/auth/v1/token?grant_type=password",
        None,
        json!({ "email": email, "password": password }),
    )
    .await
}

#[tokio::test]
async fn new_account_signs_in_with_pkce_on_both_legs() {
    let h = Harness::spawn().await;
    h.account(42, Some("Octo@Example.com"), true);

    let reply = h.authorize(APP).await;
    let provider_url = location(&reply);
    assert!(provider_url.starts_with(&format!("{AUTHORIZE}?")));
    let sent = query_of(&provider_url);
    assert_eq!(sent["client_id"], "client-1");
    assert_eq!(
        sent["redirect_uri"],
        "https://api.example.com/auth/v1/callback"
    );
    assert_eq!(sent["code_challenge_method"], "S256");
    assert_ne!(sent["code_challenge"], challenge(VERIFIER), "its own PKCE");

    let reply = h
        .callback(&format!("code=good-code&state={}", sent["state"]))
        .await;
    let back = query_of(&location(&reply));
    // Nelcota redeemed the provider's code with the verifier behind its challenge.
    let exchange = h.fake.lock().unwrap().token_requests[0].clone();
    assert_eq!(exchange["client_secret"], "client-secret-1");
    assert_eq!(
        challenge(&exchange["code_verifier"]),
        sent["code_challenge"]
    );

    let session = h.redeem(&back["code"], VERIFIER).await;
    assert_eq!(session.status, StatusCode::OK, "{}", session.text);
    let user = &session.body["user"];
    assert_eq!(user["email"], "octo@example.com");
    assert!(user["email_confirmed_at"].is_string(), "verified by GitHub");
    assert_eq!(user["user_metadata"]["name"], "Octo Cat");

    let row = h
        .app
        .admin_client
        .query_one(
            "SELECT u.encrypted_password IS NULL, i.provider, i.provider_id
               FROM auth.users u JOIN auth.identities i ON i.user_id = u.id",
            &[],
        )
        .await
        .unwrap();
    assert!(row.get::<_, bool>(0), "no password for a provider account");
    assert_eq!(row.get::<_, String>(1), "github");
    assert_eq!(row.get::<_, String>(2), "42");

    // The provider account id identifies the person, even after an email change.
    h.account(42, Some("renamed@example.com"), true);
    let again = h.session().await;
    assert_eq!(again["user"]["id"], user["id"]);
}

#[tokio::test]
async fn the_app_code_needs_its_verifier_and_works_once() {
    let h = Harness::spawn().await;
    h.account(7, Some("seven@example.com"), true);
    let back = h.sign_in().await;
    let code = &back["code"];

    let reply = h
        .redeem(code, "wrong-verifier-0123456789-abcdefghijklmnopqrstu")
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body["code"], "invalid_grant");
    // A failed attempt spends the code: a stolen code cannot be retried.
    assert_eq!(h.redeem(code, VERIFIER).await.body["code"], "invalid_grant");

    let back = h.sign_in().await;
    assert_eq!(
        h.redeem(&back["code"], VERIFIER).await.status,
        StatusCode::OK
    );
    assert_eq!(
        h.redeem(&back["code"], VERIFIER).await.body["code"],
        "invalid_grant"
    );
    let missing = h
        .app
        .post("/auth/v1/token?grant_type=pkce", None, json!({}))
        .await;
    assert_eq!(missing.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn authorize_refuses_foreign_redirects_and_weak_challenges() {
    let h = Harness::spawn().await;
    for target in [
        "https://evil.example/auth",
        "https://app.example.com/other",
        "javascript:alert(1)",
    ] {
        let reply = h.authorize(target).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{target}");
        assert_eq!(reply.body["code"], "redirect_not_allowed");
    }
    let path = format!(
        "/auth/v1/authorize?provider=github&redirect_to={APP}&code_challenge={VERIFIER}&code_challenge_method=plain"
    );
    let reply = h
        .app
        .raw(axum::http::Method::GET, &path, &[], String::new())
        .await;
    assert_eq!(reply.body["code"], "invalid_code_challenge");
    let path = format!(
        "/auth/v1/authorize?provider=google&redirect_to={APP}&code_challenge={}",
        challenge(VERIFIER)
    );
    let reply = h
        .app
        .raw(axum::http::Method::GET, &path, &[], String::new())
        .await;
    assert_eq!(
        reply.status,
        StatusCode::FORBIDDEN,
        "google is not configured"
    );
    assert_eq!(reply.body["code"], "provider_disabled");

    let plain = TestApp::spawn().await;
    let reply = plain
        .raw(axum::http::Method::GET, &path, &[], String::new())
        .await;
    assert_eq!(reply.body["code"], "provider_disabled");
}

#[tokio::test]
async fn callbacks_without_a_live_sign_in_go_nowhere() {
    let h = Harness::spawn().await;
    h.account(9, Some("nine@example.com"), true);
    let reply = h.callback("code=good-code&state=made-up").await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body["code"], "invalid_state");

    // A replayed callback finds its sign-in already answered.
    let reply = h.authorize(APP).await;
    let state = query_of(&location(&reply))["state"].clone();
    let first = h.callback(&format!("code=good-code&state={state}")).await;
    assert_eq!(first.status, StatusCode::SEE_OTHER);
    let replay = h.callback(&format!("code=good-code&state={state}")).await;
    assert_eq!(replay.body["code"], "invalid_state");

    // The person declined at the provider; the provider refused the code.
    for (query, error) in [
        ("error=access_denied", "access_denied"),
        ("code=bad-code", "provider_error"),
    ] {
        let reply = h.authorize(APP).await;
        let state = query_of(&location(&reply))["state"].clone();
        let reply = h.callback(&format!("{query}&state={state}")).await;
        assert_eq!(query_of(&location(&reply))["error"], error, "{query}");
        // The sign-in is over: its state does not work again.
        let again = h.callback(&format!("code=good-code&state={state}")).await;
        assert_eq!(again.body["code"], "invalid_state");
    }
}

#[tokio::test]
async fn verified_emails_link_confirmed_accounts() {
    let h = Harness::spawn().await;
    let signup = h
        .app
        .post(
            "/auth/v1/signup",
            None,
            json!({ "email": "linked@example.com", "password": "strong-password-123" }),
        )
        .await;
    let id = signup.body["user"]["id"].clone();
    h.app
        .admin_client
        .execute(
            "UPDATE auth.users SET email_confirmed_at = now() WHERE email = 'linked@example.com'",
            &[],
        )
        .await
        .unwrap();

    h.account(100, Some("linked@example.com"), true);
    let session = h.session().await;
    assert_eq!(session["user"]["id"], id);
    // The password the owner set keeps working.
    assert_eq!(
        password_login(&h.app, "linked@example.com", "strong-password-123")
            .await
            .status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_verified_owner_takes_over_an_unconfirmed_account() {
    let h = Harness::spawn().await;
    // Someone signed up first with an address they may not own.
    let squatter = h
        .app
        .post(
            "/auth/v1/signup",
            None,
            json!({ "email": "victim@example.com", "password": "squatter-password-1" }),
        )
        .await;
    let refresh = squatter.body["refresh_token"].as_str().unwrap().to_owned();

    h.account(200, Some("victim@example.com"), true);
    let session = h.session().await;
    assert_eq!(session["user"]["id"], squatter.body["user"]["id"]);
    assert!(session["user"]["email_confirmed_at"].is_string());

    let login = password_login(&h.app, "victim@example.com", "squatter-password-1").await;
    assert_eq!(
        login.body["code"], "invalid_grant",
        "the old password is gone"
    );
    let reply = h
        .app
        .post(
            "/auth/v1/token?grant_type=refresh_token",
            None,
            json!({ "refresh_token": refresh }),
        )
        .await;
    assert_eq!(reply.body["code"], "invalid_grant", "its sessions ended");
}

#[tokio::test]
async fn unverified_or_missing_emails_and_closed_signup_are_refused() {
    let h = Harness::spawn_with(Options {
        signup_enabled: false,
        ..Options::default()
    })
    .await;
    h.app
        .admin_client
        .execute(
            "INSERT INTO auth.users (email, email_confirmed_at) VALUES ('taken@example.com', now())",
            &[],
        )
        .await
        .unwrap();

    for (account, error) in [
        ((300, Some("taken@example.com"), false), "email_conflict"),
        ((301, None, false), "email_required"),
        ((302, Some("new@example.com"), true), "signup_disabled"),
    ] {
        let (id, email, verified) = account;
        h.account(id, email, verified);
        let back = h.sign_in().await;
        assert_eq!(back.get("error").map(String::as_str), Some(error));
        assert!(!back.contains_key("code"));
    }
    let identities: i64 = h
        .app
        .admin_client
        .query_one(
            "SELECT (SELECT count(*) FROM auth.identities) + (SELECT count(*) FROM auth.flow_states)",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(identities, 0, "refused sign-ins leave nothing behind");
}

#[tokio::test]
async fn a_duplicate_callback_does_not_void_the_code_already_issued() {
    let h = Harness::spawn().await;
    h.account(500, Some("double@example.com"), true);
    h.session().await;

    // The browser loads the provider's redirect twice at the same time; a slow
    // provider makes both read the sign-in before either finishes (with two
    // pooled connections ready, so neither waits for a new one).
    tokio::join!(h.authorize(APP), h.authorize(APP));
    h.fake.lock().unwrap().token_delay = std::time::Duration::from_millis(500);
    let reply = h.authorize(APP).await;
    let state = query_of(&location(&reply))["state"].clone();
    let query = format!("code=good-code&state={state}");
    let (first, second) = tokio::join!(h.callback(&query), h.callback(&query));
    let codes: Vec<String> = [first, second]
        .iter()
        .filter(|reply| reply.status == StatusCode::SEE_OTHER)
        .filter_map(|reply| query_of(&location(reply)).get("code").cloned())
        .collect();
    assert_eq!(codes.len(), 1, "exactly one callback hands out a code");
    assert_eq!(h.redeem(&codes[0], VERIFIER).await.status, StatusCode::OK);
}
