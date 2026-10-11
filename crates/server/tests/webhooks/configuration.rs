use crate::{
    common::{
        TestApp,
        panel::{get, login, send},
    },
    support,
};
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};

#[tokio::test]
async fn panel_configuration_is_private_validated_and_key_is_shown_once() {
    let app = TestApp::spawn().await;
    let unauthorized = app
        .request(Method::GET, "/admin/api/webhooks", None, None)
        .await;
    assert_eq!(unauthorized.status, StatusCode::UNAUTHORIZED);
    let cookie = login(&app).await;
    let value = support::body("https://example.com/hook");
    let created = send(
        &app,
        Method::POST,
        "/admin/api/webhooks",
        &cookie,
        value.clone(),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text);
    assert_eq!(created.headers[header::CACHE_CONTROL], "no-store");
    let secret = created.body["secret"].as_str().unwrap();
    assert!(secret.starts_with("whsec_"));
    assert_eq!(secret.len(), 70);
    let id = created.body["endpoint"]["id"].as_str().unwrap();
    let list = get(&app, "/admin/api/webhooks", &cookie).await;
    assert_eq!(list.body["endpoints"][0]["id"], id);
    assert!(!list.text.contains(secret));
    assert!(!list.text.contains("signing_secret"));
    assert_eq!(list.body["outstanding"], 0);
    assert_eq!(list.body["allow_loopback"], false);
    let mut paused = value.clone();
    paused["enabled"] = json!(false);
    let saved = send(
        &app,
        Method::PUT,
        &format!("/admin/api/webhooks/{id}"),
        &cookie,
        paused,
    )
    .await;
    assert_eq!(saved.body["enabled"], false);
    assert!(!saved.text.contains(secret));
    let rotated = send(
        &app,
        Method::POST,
        &format!("/admin/api/webhooks/{id}/rotate"),
        &cookie,
        json!({}),
    )
    .await;
    assert_eq!(rotated.status, StatusCode::OK);
    assert_ne!(rotated.body["secret"], secret);
    assert_eq!(rotated.headers[header::CACHE_CONTROL], "no-store");
    let malicious = app
        .raw(
            Method::PUT,
            &format!("/admin/api/webhooks/{id}"),
            &[
                ("cookie", &cookie),
                ("content-type", "application/json"),
                ("origin", "https://attacker.example"),
            ],
            value.to_string(),
        )
        .await;
    assert_eq!(malicious.status, StatusCode::FORBIDDEN);
    let deleted = send(
        &app,
        Method::DELETE,
        &format!("/admin/api/webhooks/{id}"),
        &cookie,
        json!({}),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert!(
        get(&app, "/admin/api/webhooks", &cookie).await.body["endpoints"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let cleared: bool = app
        .admin_client
        .query_one(
            "SELECT signing_secret IS NULL FROM nelcota.webhook_endpoints WHERE id=$1::text::uuid",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert!(cleared);
    let trigger: bool=app.admin_client.query_one("SELECT EXISTS (SELECT FROM pg_trigger WHERE tgrelid='public.todos'::regclass AND tgname='nelcota_webhook_events')", &[]).await.unwrap().get(0);
    assert!(!trigger);
}

#[tokio::test]
async fn invalid_configuration_cannot_create_triggers_or_inject_sql() {
    let app = TestApp::spawn().await;
    let pool = nelcota_core::db::admin_pool(&app.admin, 2);
    for schema in ["auth", "storage", "nelcota", "pg_catalog"] {
        assert!(
            nelcota_webhooks::management::tables(&pool, schema)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            nelcota_webhooks::management::create(
                &pool,
                schema,
                support::input("https://example.com/hook"),
                Default::default()
            )
            .await
            .is_err()
        );
    }
    let cookie = login(&app).await;
    for url in [
        "http://example.com/",
        "https://127.0.0.1/",
        "https://169.254.169.254/",
        "https://10.0.0.1/",
        "https://[::ffff:127.0.0.1]/",
        "https://user:secret@example.com/",
        "https://example.com/?key=secret",
    ] {
        let reply = send(
            &app,
            Method::POST,
            "/admin/api/webhooks",
            &cookie,
            support::body(url),
        )
        .await;
        assert_eq!(
            reply.status,
            StatusCode::BAD_REQUEST,
            "{url}: {}",
            reply.text
        );
        assert_eq!(reply.body["code"], "webhook_invalid_url");
    }
    let cases: Vec<(&str, Value)> = vec![
        ("table", json!("todos\"; DROP TABLE public.todos; --")),
        ("table", json!("auth.users")),
        ("name", json!("")),
        ("events", json!([])),
        ("events", json!(["TRUNCATE"])),
        ("max_attempts", json!(0)),
        ("max_attempts", json!(11)),
    ];
    for (field, value) in cases {
        let mut body = support::body("https://example.com/hook");
        body[field] = value;
        assert_eq!(
            send(&app, Method::POST, "/admin/api/webhooks", &cookie, body)
                .await
                .status,
            StatusCode::BAD_REQUEST
        );
    }
    let mut unknown = support::body("https://example.com/hook");
    unknown["signing_secret"] = json!("attacker-chosen-key");
    assert_eq!(
        send(&app, Method::POST, "/admin/api/webhooks", &cookie, unknown)
            .await
            .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        app.admin_client
            .query_one("SELECT count(*) FROM public.todos", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        2
    );
    assert_eq!(support::count(&app).await, 0);
}
