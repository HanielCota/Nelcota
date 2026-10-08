//! Shared panel session and request helpers for integration suites.
use super::{ADMIN_EMAIL, ADMIN_PASSWORD, Reply, TestApp};
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};

pub const JSON: (&str, &str) = ("content-type", "application/json");

pub async fn login(app: &TestApp) -> String {
    let reply = app
        .raw(
            Method::POST,
            "/admin/api/login",
            &[JSON],
            json!({ "email": ADMIN_EMAIL, "password": ADMIN_PASSWORD }).to_string(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let cookie = reply.headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    cookie.split(';').next().unwrap().to_owned()
}

pub async fn get(app: &TestApp, path: &str, cookie: &str) -> Reply {
    app.raw(Method::GET, path, &[("cookie", cookie)], String::new())
        .await
}

pub async fn send(app: &TestApp, method: Method, path: &str, cookie: &str, body: Value) -> Reply {
    app.raw(method, path, &[("cookie", cookie), JSON], body.to_string())
        .await
}

pub async fn sql(app: &TestApp, cookie: &str, query: &str) -> Value {
    sql_response(app, cookie, query).await.body
}

/// SQL HTTP response, including status and headers for transport assertions.
pub async fn sql_response(app: &TestApp, cookie: &str, query: &str) -> Reply {
    send(
        app,
        Method::POST,
        "/admin/api/sql",
        cookie,
        json!({ "sql": query }),
    )
    .await
}
