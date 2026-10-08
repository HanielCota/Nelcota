//! Panel profile integration scenarios.
use crate::common::panel::{JSON, get, login, send, sql};
use crate::common::*;
use axum::http::{Method, StatusCode, header};
use serde_json::json;

#[tokio::test]
async fn admin_profile_photo() {
    // Minimal PNG (signature + start of IHDR) and an SVG, in base64.
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUg==";
    const SVG: &str = "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=";
    let app = TestApp::spawn().await;

    for path in ["/admin/api/profile", "/admin/api/profile/avatar"] {
        let reply = app.raw(Method::GET, path, &[], String::new()).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{path}");
    }

    let cookie = login(&app).await;
    let profile = get(&app, "/admin/api/profile", &cookie).await;
    assert_eq!(profile.status, StatusCode::OK, "{}", profile.text);
    assert_eq!(profile.body["email"], ADMIN_EMAIL);
    assert!(profile.body["avatar"].is_null());
    let reply = get(&app, "/admin/api/profile/avatar", &cookie).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);

    // Only PNG, JPEG or WebP recognised by their bytes; size is limited.
    let too_big = "A".repeat(349_528); // 262,146 decoded bytes
    for image in [SVG, "not base64!", "", too_big.as_str()] {
        let reply = send(
            &app,
            Method::PUT,
            "/admin/api/profile/avatar",
            &cookie,
            json!({ "image": image }),
        )
        .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{}", reply.text);
    }

    // CSRF: another origin does not change the photo.
    let reply = app
        .raw(
            Method::PUT,
            "/admin/api/profile/avatar",
            &[
                ("cookie", &cookie),
                JSON,
                ("origin", "https://malicious.example"),
            ],
            json!({ "image": PNG }).to_string(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);

    let reply = send(
        &app,
        Method::PUT,
        "/admin/api/profile/avatar",
        &cookie,
        json!({ "image": PNG }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let version = reply.body["avatar"].as_i64().expect("photo version");
    let profile = get(&app, "/admin/api/profile", &cookie).await;
    assert_eq!(profile.body["avatar"], version);

    let reply = get(&app, "/admin/api/profile/avatar", &cookie).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(reply.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(reply.text.ends_with("IHDR"));

    // The table is out of reach of the API roles.
    let out = sql(
        &app,
        &cookie,
        "select has_schema_privilege('anon', 'nelcota', 'usage'),
                has_schema_privilege('authenticated', 'nelcota', 'usage'),
                has_schema_privilege('service_role', 'nelcota', 'usage')",
    )
    .await;
    assert_eq!(out["results"][0]["rows"], json!([["f", "f", "f"]]));

    let reply = send(
        &app,
        Method::DELETE,
        "/admin/api/profile/avatar",
        &cookie,
        json!({}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.body["avatar"].is_null());
    let reply = get(&app, "/admin/api/profile/avatar", &cookie).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}
