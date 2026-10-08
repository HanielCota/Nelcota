//! Authentication flow tests against a real Postgres: signup, login, refresh
//! with rotation and reuse detection, logout, JWKS, rate limit and password
//! recovery by email.

mod common;

use axum::http::{StatusCode, header};
use common::*;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, jwk::JwkSet};
use nelcota_core::{Claims, db};
use serde_json::{Value, json};
use uuid::Uuid;

async fn signup(app: &TestApp, email: &str, password: &str) -> Reply {
    app.post(
        "/auth/v1/signup",
        None,
        json!({ "email": email, "password": password }),
    )
    .await
}

async fn login(app: &TestApp, email: &str, password: &str) -> Reply {
    app.post(
        "/auth/v1/token?grant_type=password",
        None,
        json!({ "email": email, "password": password }),
    )
    .await
}

async fn refresh(app: &TestApp, refresh_token: &str) -> Reply {
    app.post(
        "/auth/v1/token?grant_type=refresh_token",
        None,
        json!({ "refresh_token": refresh_token }),
    )
    .await
}

fn str_field<'a>(body: &'a Value, field: &str) -> &'a str {
    body[field]
        .as_str()
        .unwrap_or_else(|| panic!("campo {field} ausente em {body}"))
}

#[tokio::test]
async fn full_flow_signup_login_and_access_under_rls() {
    let app = TestApp::spawn().await;

    let reply = signup(&app, "Ana@Example.com", "strong-pass-123").await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    let session = reply.body;
    assert_eq!(session["token_type"], "bearer");
    assert_eq!(session["user"]["email"], "ana@example.com");
    let user_id: Uuid = str_field(&session["user"], "id").parse().unwrap();

    // The issued JWT is EdDSA and carries the contract's claims.
    let access = str_field(&session, "access_token");
    let header = jsonwebtoken::decode_header(access).unwrap();
    assert_eq!(header.alg, Algorithm::EdDSA);
    assert!(header.kid.is_some());

    let (status, me) = app.get("/auth/v1/user", Some(access)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["id"], user_id.to_string());

    // The auth token works in the API under RLS: Ana only sees her own todo.
    app.admin_client
        .execute(
            "INSERT INTO public.todos (user_id, title) VALUES ($1, 'Ana''s todo')",
            &[&user_id],
        )
        .await
        .unwrap();
    let (status, todos) = app.get("/rest/v1/todos", Some(access)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(todos, json!([todos[0].clone()]));
    assert_eq!(todos[0]["title"], "Ana's todo");

    // Login with the same password (email in a different case) opens another session.
    let reply = login(&app, "ANA@example.com", "strong-pass-123").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_ne!(
        str_field(&reply.body, "refresh_token"),
        str_field(&session, "refresh_token")
    );
    assert!(reply.body["user"]["last_sign_in_at"].is_string());
}

#[tokio::test]
async fn password_stored_as_phc_argon2id() {
    let app = TestApp::spawn().await;
    assert_eq!(
        signup(&app, "bia@example.com", "strong-pass-123")
            .await
            .status,
        StatusCode::CREATED
    );
    let hash: String = app
        .admin_client
        .query_one(
            "SELECT encrypted_password FROM auth.users WHERE email = 'bia@example.com'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(hash.starts_with("$argon2id$v=19$"), "{hash}");
    assert!(!hash.contains("strong-pass-123"));

    // Refresh tokens are not stored in the clear either.
    let stored: i64 = app
        .admin_client
        .query_one(
            "SELECT count(*) FROM auth.refresh_tokens WHERE length(token_hash) = 32",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored, 1);
}

#[tokio::test]
async fn signup_validates_input_and_duplicate_email() {
    let app = TestApp::spawn().await;
    assert_eq!(
        signup(&app, "caio@example.com", "strong-pass-123")
            .await
            .status,
        StatusCode::CREATED
    );
    let dup = signup(&app, "CAIO@example.com", "other-pass-456").await;
    assert_eq!(dup.status, StatusCode::CONFLICT);
    assert_eq!(dup.body["code"], "user_already_exists");

    assert_eq!(
        signup(&app, "not-an-email", "strong-pass-123").await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        signup(&app, "dani@example.com", "short").await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn invalid_login_does_not_reveal_whether_email_exists() {
    let app = TestApp::spawn().await;
    signup(&app, "edu@example.com", "strong-pass-123").await;

    let wrong = login(&app, "edu@example.com", "wrong-pass-000").await;
    let unknown = login(&app, "nobody@example.com", "wrong-pass-000").await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);
    assert_eq!(wrong.body, unknown.body);
    assert_eq!(wrong.body["code"], "invalid_grant");
}

#[tokio::test]
async fn refresh_rotates_and_reuse_voids_the_family() {
    let app = TestApp::spawn().await;
    let session = signup(&app, "fabi@example.com", "strong-pass-123")
        .await
        .body;
    let r1 = str_field(&session, "refresh_token").to_owned();

    // Legitimate use: R1 -> R2.
    let rotated = refresh(&app, &r1).await;
    assert_eq!(rotated.status, StatusCode::OK, "{}", rotated.body);
    let r2 = str_field(&rotated.body, "refresh_token").to_owned();
    assert_ne!(r1, r2);
    assert!(rotated.body["access_token"].is_string());

    // R2 -> R3 keeps working.
    let rotated = refresh(&app, &r2).await;
    assert_eq!(rotated.status, StatusCode::OK);
    let r3 = str_field(&rotated.body, "refresh_token").to_owned();

    // Reuse of R1 (stolen): rejected and the whole session is revoked...
    let reuse = refresh(&app, &r1).await;
    assert_eq!(reuse.status, StatusCode::BAD_REQUEST);
    assert_eq!(reuse.body["code"], "invalid_grant");

    // ...including the newest token of the family.
    let after = refresh(&app, &r3).await;
    assert_eq!(after.status, StatusCode::BAD_REQUEST);

    // Other sessions of the same user are not affected.
    let other = login(&app, "fabi@example.com", "strong-pass-123")
        .await
        .body;
    assert_eq!(
        refresh(&app, str_field(&other, "refresh_token"))
            .await
            .status,
        StatusCode::OK
    );

    assert_eq!(
        refresh(&app, "no-such-token").await.status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn logout_ends_the_session() {
    let app = TestApp::spawn().await;
    let session = signup(&app, "gabi@example.com", "strong-pass-123")
        .await
        .body;
    let access = str_field(&session, "access_token");

    let reply = app.post("/auth/v1/logout", Some(access), json!({})).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(
        refresh(&app, str_field(&session, "refresh_token"))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );

    // Logout requires a user JWT.
    let reply = app.post("/auth/v1/logout", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    let reply = app
        .post("/auth/v1/logout", Some(&service_token()), json!({}))
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn jwks_allows_validating_tokens_outside_nelcota() {
    let app = TestApp::spawn().await;
    let session = signup(&app, "hugo@example.com", "strong-pass-123")
        .await
        .body;

    let reply = app
        .request(
            axum::http::Method::GET,
            "/auth/v1/.well-known/jwks.json",
            None,
            None,
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.headers.contains_key(header::CACHE_CONTROL));
    let jwks: JwkSet = serde_json::from_value(reply.body).unwrap();
    assert_eq!(jwks.keys.len(), 1);

    // An external service validates the token with the public key alone.
    let key = DecodingKey::from_jwk(&jwks.keys[0]).unwrap();
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_audience(&["authenticated"]);
    validation.set_issuer(&["nelcota-test"]);
    let data =
        jsonwebtoken::decode::<Value>(str_field(&session, "access_token"), &key, &validation)
            .unwrap();
    assert_eq!(data.claims["role"], "authenticated");
    assert_eq!(data.claims["email"], "hugo@example.com");
    assert!(data.claims["session_id"].is_string());
}

#[tokio::test]
async fn rate_limit_on_login() {
    let app = TestApp::spawn_with(Options {
        rate_limit_per_minute: 3,
        ..Options::default()
    })
    .await;
    for _ in 0..3 {
        let reply = login(&app, "ivo@example.com", "wrong-pass-000").await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    }
    let blocked = login(&app, "ivo@example.com", "wrong-pass-000").await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(blocked.headers.contains_key(header::RETRY_AFTER));
}

/// The `auth.*` tables are not accessible to the API roles.
#[tokio::test]
async fn api_roles_cannot_read_the_auth_schema() {
    let app = TestApp::spawn().await;
    signup(&app, "julia@example.com", "strong-pass-123").await;

    for payload in [
        json!({ "role": "anon" }),
        json!({ "role": "authenticated", "sub": app.user_a }),
        json!({ "role": "service_role" }),
    ] {
        let claims = Claims::from_payload(payload.clone()).unwrap();
        for query in [
            "SELECT encrypted_password FROM auth.users",
            "SELECT token_hash FROM auth.one_time_tokens",
        ] {
            // One transaction per query: the error aborts the whole transaction.
            let mut client = app.pool.get().await.unwrap();
            let tx = db::begin_request(&mut client, &claims).await.unwrap();
            let err = tx.query(query, &[]).await.unwrap_err();
            assert_eq!(
                err.code(),
                Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE),
                "{payload}: {query}"
            );
        }
    }

    // And no JWT can assume the internal auth role.
    let (status, _) = app
        .get(
            "/rest/v1/todos",
            Some(&token(json!({ "role": "nelcota_auth" }))),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------- password recovery

async fn recover(app: &TestApp, email: &str) -> Reply {
    app.post("/auth/v1/recover", None, json!({ "email": email }))
        .await
}

async fn verify(app: &TestApp, token: &str, password: &str) -> Reply {
    app.post(
        "/auth/v1/verify",
        None,
        json!({ "type": "recovery", "token": token, "password": password }),
    )
    .await
}

/// Token of the email link (`...#type=recovery&token=<token>`).
fn link_token(email: &nelcota_auth::Email) -> String {
    let link = email
        .text
        .split_whitespace()
        .find(|word| word.starts_with(RECOVERY_URL))
        .unwrap_or_else(|| panic!("sem link em: {}", email.text));
    link.strip_prefix(&format!("{RECOVERY_URL}#type=recovery&token="))
        .unwrap_or_else(|| panic!("link fora do formato: {link}"))
        .to_owned()
}

#[tokio::test]
async fn password_recovery_changes_the_password_and_ends_sessions() {
    let app = TestApp::spawn().await;
    let old = signup(&app, "ana@example.com", "old-pass-123").await.body;
    let old_refresh = str_field(&old, "refresh_token").to_owned();
    assert!(old["user"]["email_confirmed_at"].is_null());

    let reply = recover(&app, "Ana@Example.com").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body, json!({}));
    let sent = app.outbox.wait_for(1).await;
    assert_eq!(sent[0].to, "ana@example.com");
    let token = link_token(&sent[0]);

    // Only the token's SHA-256 is stored in the database.
    let row = app
        .admin_client
        .query_one(
            "SELECT count(*) FILTER (WHERE token_hash = sha256($1::text::bytea)),
                    count(*) FILTER (WHERE position($1::text::bytea IN token_hash) > 0)
             FROM auth.one_time_tokens",
            &[&token],
        )
        .await
        .unwrap();
    assert_eq!((row.get::<_, i64>(0), row.get::<_, i64>(1)), (1, 0));

    let reply = verify(&app, &token, "new-pass-456").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["user"]["email"], "ana@example.com");
    // Opening the link proves the person receives the account's emails.
    assert!(reply.body["user"]["email_confirmed_at"].is_string());
    assert!(reply.body["access_token"].is_string());

    assert_eq!(
        login(&app, "ana@example.com", "old-pass-123").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        login(&app, "ana@example.com", "new-pass-456").await.status,
        StatusCode::OK
    );
    // The session opened with the old password was ended.
    let reply = refresh(&app, &old_refresh).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body["code"], "invalid_grant");

    // The link works only once.
    let reply = verify(&app, &token, "other-pass-789").await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body["code"], "invalid_grant");
}

#[tokio::test]
async fn concurrent_recovery_submissions_consume_the_link_once() {
    let app = TestApp::spawn().await;
    signup(&app, "concurrent@example.com", "old-password-123").await;
    recover(&app, "concurrent@example.com").await;
    let sent = app.outbox.wait_for(1).await;
    let token = link_token(&sent[0]);
    let (first, second) = tokio::join!(
        verify(&app, &token, "first-password-123"),
        verify(&app, &token, "second-password-123")
    );
    let (winner, loser, password) = if first.status == StatusCode::OK {
        (first, second, "first-password-123")
    } else {
        (second, first, "second-password-123")
    };
    assert_eq!(winner.status, StatusCode::OK);
    assert_eq!(loser.status, StatusCode::BAD_REQUEST);
    assert_eq!(loser.body["code"], "invalid_grant");
    assert_eq!(
        login(&app, "concurrent@example.com", password).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn recovery_does_not_reveal_whether_the_account_exists() {
    let app = TestApp::spawn().await;
    signup(&app, "bia@example.com", "strong-pass-123").await;

    let existing = recover(&app, "bia@example.com").await;
    let missing = recover(&app, "nobody@example.com").await;
    assert_eq!(
        (existing.status, &existing.body),
        (missing.status, &missing.body)
    );
    app.outbox.wait_for(1).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(
        app.outbox.sent().len(),
        1,
        "only the existing account receives it"
    );

    let reply = recover(&app, "not an email").await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn recovery_sends_one_email_per_minute_and_only_the_last_link_works() {
    let app = TestApp::spawn().await;
    signup(&app, "caio@example.com", "strong-pass-123").await;

    recover(&app, "caio@example.com").await;
    let first = link_token(&app.outbox.wait_for(1).await[0]);
    // A repeated request right away: same response, no new email.
    assert_eq!(
        recover(&app, "caio@example.com").await.status,
        StatusCode::OK
    );
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(app.outbox.sent().len(), 1);

    // After the interval, a new request sends another link and voids the previous one.
    app.admin_client
        .execute(
            "UPDATE auth.one_time_tokens SET created_at = now() - interval '2 minutes'",
            &[],
        )
        .await
        .unwrap();
    recover(&app, "caio@example.com").await;
    let second = link_token(&app.outbox.wait_for(2).await[1]);
    assert_ne!(first, second);
    assert_eq!(
        verify(&app, &first, "new-pass-456").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        verify(&app, &second, "new-pass-456").await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn expired_or_invalid_link_or_weak_password() {
    let app = TestApp::spawn().await;
    signup(&app, "davi@example.com", "strong-pass-123").await;
    recover(&app, "davi@example.com").await;
    let token = link_token(&app.outbox.wait_for(1).await[0]);

    // A password breaking the rules does not consume the link.
    let reply = verify(&app, &token, "short").await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);

    for bad in ["", "made-up-token", &"x".repeat(200)] {
        let reply = verify(&app, bad, "new-pass-456").await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let reply = app
        .post(
            "/auth/v1/verify",
            None,
            json!({ "type": "signup", "token": token, "password": "new-pass-456" }),
        )
        .await;
    assert_eq!(reply.body["code"], "unsupported_type");

    app.admin_client
        .execute(
            "UPDATE auth.one_time_tokens SET expires_at = now() - interval '1 second'",
            &[],
        )
        .await
        .unwrap();
    let reply = verify(&app, &token, "new-pass-456").await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        login(&app, "davi@example.com", "strong-pass-123")
            .await
            .status,
        StatusCode::OK,
        "the old password still works"
    );
}

#[tokio::test]
async fn recovery_disabled_without_smtp() {
    let app = TestApp::spawn_with(Options {
        mail: false,
        ..Options::default()
    })
    .await;
    signup(&app, "eva@example.com", "strong-pass-123").await;
    let reply = recover(&app, "eva@example.com").await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.body["code"], "recovery_disabled");
}
