//! Panel users integration scenarios.
use crate::common::panel::{JSON, get, login, send};
use crate::common::*;
use axum::http::{Method, StatusCode};
use serde_json::json;

#[tokio::test]
async fn users_and_sessions() {
    let app = TestApp::spawn().await;
    let session = app
        .post(
            "/auth/v1/signup",
            None,
            json!({ "email": "panel@example.com", "password": "strong-password-123" }),
        )
        .await
        .body;
    let user_id = session["user"]["id"].as_str().unwrap().to_owned();
    let cookie = login(&app).await;

    let users = get(&app, "/admin/api/users?q=panel", &cookie).await;
    assert_eq!(users.body["users"][0]["email"], "panel@example.com");
    assert_eq!(users.body["users"][0]["sessions"], 1);
    assert!(
        !users.text.contains("argon2id"),
        "the hash never leaves the database"
    );

    let reply = send(
        &app,
        Method::POST,
        &format!("/admin/api/users/{user_id}/revoke"),
        &cookie,
        json!({}),
    )
    .await;
    assert_eq!(reply.body["count"], 1);
    let refresh = app
        .post(
            "/auth/v1/token?grant_type=refresh_token",
            None,
            json!({ "refresh_token": session["refresh_token"] }),
        )
        .await;
    assert_eq!(refresh.status, StatusCode::BAD_REQUEST);

    let reply = send(
        &app,
        Method::DELETE,
        &format!("/admin/api/users/{user_id}"),
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    let users = get(&app, "/admin/api/users", &cookie).await;
    assert!(!users.text.contains("panel@example.com"));

    let reply = send(
        &app,
        Method::DELETE,
        "/admin/api/users/not-a-uuid",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
}

async fn password_login(app: &TestApp, email: &str, password: &str) -> Reply {
    app.raw(
        Method::POST,
        "/auth/v1/token?grant_type=password",
        &[JSON],
        json!({ "email": email, "password": password }).to_string(),
    )
    .await
}

#[tokio::test]
async fn create_user_and_reset_password_from_the_panel() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let created = send(
        &app,
        Method::POST,
        "/admin/api/users",
        &cookie,
        json!({ "email": "  New@Example.com ", "password": "initial-password-123" }),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text);
    // Same rules as public signup: normalized email.
    assert_eq!(created.body["email"], "new@example.com");
    let id = created.body["id"].as_str().unwrap().to_owned();

    // The created user signs in through the public API, with the same hash as signup.
    let session = password_login(&app, "new@example.com", "initial-password-123").await;
    assert_eq!(session.status, StatusCode::OK, "{}", session.text);
    let refresh = session.body["refresh_token"].as_str().unwrap().to_owned();

    for (body, status, code) in [
        (
            json!({ "email": "new@example.com", "password": "other-password-123" }),
            StatusCode::CONFLICT,
            "user_already_exists",
        ),
        (
            json!({ "email": "no-at-sign", "password": "valid-password-123" }),
            StatusCode::BAD_REQUEST,
            "invalid_email",
        ),
        (
            json!({ "email": "short@example.com", "password": "1234567" }),
            StatusCode::BAD_REQUEST,
            "password_too_short",
        ),
    ] {
        let reply = send(&app, Method::POST, "/admin/api/users", &cookie, body).await;
        assert_eq!(reply.status, status, "{}", reply.text);
        assert_eq!(reply.body["code"], code, "{}", reply.text);
    }

    // Reset: the old password stops working and the open session is ended.
    let reset = send(
        &app,
        Method::PUT,
        &format!("/admin/api/users/{id}/password"),
        &cookie,
        json!({ "password": "new-password-456" }),
    )
    .await;
    assert_eq!(reset.status, StatusCode::OK, "{}", reset.text);
    assert_eq!(reset.body["sessions_revoked"], 1);
    assert_eq!(
        password_login(&app, "new@example.com", "initial-password-123")
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        password_login(&app, "new@example.com", "new-password-456")
            .await
            .status,
        StatusCode::OK
    );
    let reused = app
        .raw(
            Method::POST,
            "/auth/v1/token?grant_type=refresh_token",
            &[JSON],
            json!({ "refresh_token": refresh }).to_string(),
        )
        .await;
    assert_ne!(
        reused.status,
        StatusCode::OK,
        "a session from before the password change cannot refresh"
    );

    let weak = send(
        &app,
        Method::PUT,
        &format!("/admin/api/users/{id}/password"),
        &cookie,
        json!({ "password": "short" }),
    )
    .await;
    assert_eq!(weak.status, StatusCode::BAD_REQUEST);
    let missing = send(
        &app,
        Method::PUT,
        "/admin/api/users/00000000-0000-0000-0000-000000000000/password",
        &cookie,
        json!({ "password": "valid-password-123" }),
    )
    .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.body["code"], "user_not_found");
    let bad_id = send(
        &app,
        Method::PUT,
        "/admin/api/users/not-a-uuid/password",
        &cookie,
        json!({ "password": "valid-password-123" }),
    )
    .await;
    assert_eq!(bad_id.status, StatusCode::BAD_REQUEST);
}
