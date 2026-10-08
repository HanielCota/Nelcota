//! Panel policies integration scenarios.
use crate::common::panel::{get, login, send};
use crate::common::*;
use axum::http::{Method, StatusCode};
use serde_json::json;

async fn anon_rows(app: &TestApp) -> usize {
    let reply = app
        .raw(Method::GET, "/rest/v1/products", &[], String::new())
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    reply.body.as_array().unwrap().len()
}

#[tokio::test]
async fn policies_created_edited_and_dropped_from_the_panel() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    assert_eq!(
        anon_rows(&app).await,
        4,
        "without RLS, anon sees everything"
    );

    let rls = send(
        &app,
        Method::PATCH,
        "/admin/api/tables/products",
        &cookie,
        json!({ "actions": [{ "action": "set_rls", "enabled": true }] }),
    )
    .await;
    assert_eq!(rls.status, StatusCode::OK, "{}", rls.text);
    assert_eq!(
        anon_rows(&app).await,
        0,
        "RLS without policies: nobody sees anything"
    );

    let policy =
        json!({ "name": "public read", "command": "select", "roles": ["anon"], "using": "true" });
    let created = send(
        &app,
        Method::POST,
        "/admin/api/tables/products/policies",
        &cookie,
        json!({ "policy": policy }),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text);
    assert_eq!(anon_rows(&app).await, 4);

    // Edit: changes the expression and the name in one transaction (DROP + CREATE).
    let edited = send(&app, Method::PUT, "/admin/api/tables/products/policies/public%20read", &cookie,
        json!({ "policy": { "name": "in stock", "command": "select", "roles": ["anon"], "using": "stock > 0" } })).await;
    assert_eq!(edited.status, StatusCode::OK, "{}", edited.text);
    assert_eq!(anon_rows(&app).await, 3, "Backpack has stock 0");
    let listed = get(&app, "/admin/api/policies", &cookie).await.body;
    let products = listed["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "products")
        .unwrap()
        .clone();
    assert_eq!(products["policies"][0]["name"], "in stock");
    assert_eq!(products["policies"][0]["using"], "(stock > 0)");

    // Each command's rules are validated before the database.
    let invalid = send(
        &app,
        Method::POST,
        "/admin/api/tables/products/policies",
        &cookie,
        json!({ "policy": { "name": "x", "command": "insert", "using": "true" } }),
    )
    .await;
    assert_eq!(invalid.status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid.body["code"], "policy_insert_no_using");
    // A second statement hidden in the expression: refused, and nothing changes.
    let injection = send(&app, Method::POST, "/admin/api/tables/products/policies", &cookie,
        json!({ "policy": { "name": "y", "command": "select", "using": "true) ; DROP TABLE public.products; --" } })).await;
    assert_eq!(
        injection.status,
        StatusCode::BAD_REQUEST,
        "{}",
        injection.text
    );
    assert_eq!(anon_rows(&app).await, 3);

    let dropped = app
        .raw(
            Method::DELETE,
            "/admin/api/tables/products/policies/in%20stock",
            &[("cookie", cookie.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(dropped.status, StatusCode::OK, "{}", dropped.text);
    assert_eq!(anon_rows(&app).await, 0);
    let missing = app
        .raw(
            Method::DELETE,
            "/admin/api/tables/products/policies/missing",
            &[("cookie", cookie.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST, "{}", missing.text);
}
