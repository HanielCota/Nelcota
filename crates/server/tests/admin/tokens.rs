//! Panel tokens integration scenarios.
use crate::common::panel::{JSON, login, send};
use crate::common::*;
use axum::http::{Method, StatusCode, header};
use serde_json::json;

#[tokio::test]
async fn service_role_token_issued_by_the_panel() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let anon = app
        .raw(Method::GET, "/rest/v1/secrets", &[], String::new())
        .await;
    assert_ne!(
        anon.status,
        StatusCode::OK,
        "secrets only has a GRANT for service_role"
    );

    let reply = send(
        &app,
        Method::POST,
        "/admin/api/tokens/service-role",
        &cookie,
        json!({ "days": 7 }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");
    let token = reply.body["token"].as_str().unwrap().to_owned();
    let bearer = format!("Bearer {token}");
    let service = app
        .raw(
            Method::GET,
            "/rest/v1/secrets",
            &[("authorization", bearer.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(service.status, StatusCode::OK, "{}", service.text);

    for days in [0, 3651] {
        let bad = send(
            &app,
            Method::POST,
            "/admin/api/tokens/service-role",
            &cookie,
            json!({ "days": days }),
        )
        .await;
        assert_eq!(bad.status, StatusCode::BAD_REQUEST);
        assert_eq!(bad.body["code"], "invalid_token_validity");
        assert_eq!(bad.body["params"], json!({ "max": 3650 }));
    }
    // Without a panel session, no token.
    let outsider = app
        .raw(
            Method::POST,
            "/admin/api/tokens/service-role",
            &[JSON],
            json!({ "days": 7 }).to_string(),
        )
        .await;
    assert_eq!(outsider.status, StatusCode::UNAUTHORIZED);
}
