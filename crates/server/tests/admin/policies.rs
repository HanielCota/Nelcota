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

#[tokio::test]
async fn guided_table_and_policy_access_are_applied_atomically() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    let specification = json!({
        "name": "guided_notes", "rls": true,
        "columns": [
            { "name": "id", "data_type": "bigint", "identity": true, "primary_key": true },
            { "name": "body", "data_type": "text" },
            { "name": "user_id", "data_type": "uuid", "nullable": false, "default": "auth.uid()" }
        ],
        "grants": [
            { "role": "service_role", "privileges": ["select", "insert", "update", "delete"] },
            { "role": "authenticated", "privileges": ["select", "insert", "update", "delete"] }
        ]
    });
    let definition = json!({ "name": "owner_all", "command": "all", "roles": ["authenticated"], "using": "user_id = auth.uid()", "check": "user_id = auth.uid()" });
    let preview = send(
        &app,
        Method::POST,
        "/admin/api/tables",
        &cookie,
        json!({ "table": specification, "policies": [definition], "preview": true }),
    )
    .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.text);
    assert!(
        preview.body["sql"]
            .as_array()
            .unwrap()
            .iter()
            .any(|sql| sql.as_str().unwrap().contains("CREATE POLICY"))
    );
    assert!(app.catalog.get().table("guided_notes").is_none());
    let created = send(
        &app,
        Method::POST,
        "/admin/api/tables",
        &cookie,
        json!({ "table": specification, "policies": [definition] }),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text);
    app.admin_client
        .execute(
            "INSERT INTO public.guided_notes (body, user_id) VALUES ('mine', $1), ('other', $2)",
            &[&app.user_a, &app.user_b],
        )
        .await
        .unwrap();
    let bearer = format!(
        "Bearer {}",
        token(json!({ "sub": app.user_a, "role": "authenticated" }))
    );
    let own = app
        .raw(
            Method::GET,
            "/rest/v1/guided_notes",
            &[("authorization", bearer.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(own.status, StatusCode::OK, "{}", own.text);
    assert_eq!(
        own.body.as_array().unwrap().len(),
        1,
        "owner rule hides the other user's row"
    );
    let inserted = app
        .raw(
            Method::POST,
            "/rest/v1/guided_notes",
            &[
                ("authorization", bearer.as_str()),
                ("content-type", "application/json"),
            ],
            json!({ "body": "automatic owner" }).to_string(),
        )
        .await;
    assert_eq!(inserted.status, StatusCode::CREATED, "{}", inserted.text);
    let owner: uuid::Uuid = app
        .admin_client
        .query_one(
            "SELECT user_id FROM public.guided_notes WHERE body = 'automatic owner'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(owner, app.user_a);

    // Guided policy creation activates protection and fills missing grants.
    let policy = send(&app, Method::POST, "/admin/api/tables/guided_notes/policies", &cookie, json!({
        "policy": { "name": "public_read", "command": "select", "roles": ["anon"], "using": "true" }, "prepare_access": true
    })).await;
    assert_eq!(policy.status, StatusCode::OK, "{}", policy.text);
    let read = app
        .raw(Method::GET, "/rest/v1/guided_notes", &[], String::new())
        .await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.text);
    assert_eq!(read.body.as_array().unwrap().len(), 3);
    let structure = get(&app, "/admin/api/tables/guided_notes/structure", &cookie)
        .await
        .body;
    let grants = structure["grants"].as_array().unwrap();
    assert_eq!(
        grants.iter().find(|grant| grant["role"] == "anon").unwrap()["privileges"],
        json!(["select"])
    );
    assert_eq!(
        grants
            .iter()
            .find(|grant| grant["role"] == "service_role")
            .unwrap()["privileges"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    // A duplicate name fails inside the transaction: even CREATE TABLE rolls back.
    let failed = send(&app, Method::POST, "/admin/api/tables", &cookie, json!({
        "table": { "name": "guided_rollback", "columns": [{ "name": "id", "data_type": "bigint" }, { "name": "user_id", "data_type": "uuid" }] },
        "policies": [definition, definition]
    })).await;
    assert_eq!(failed.status, StatusCode::BAD_REQUEST, "{}", failed.text);
    let absent: Option<String> = app
        .admin_client
        .query_one("SELECT to_regclass('public.guided_rollback')::text", &[])
        .await
        .unwrap()
        .get(0);
    assert!(absent.is_none());

    // A missing owner field is added with the policy, without claiming old rows.
    let private = send(
        &app,
        Method::POST,
        "/admin/api/tables",
        &cookie,
        json!({ "table": {
        "name": "guided_auto_owner", "columns": [
            { "name": "id", "data_type": "bigint", "identity": true, "primary_key": true },
            { "name": "body", "data_type": "text" }
        ]
    } }),
    )
    .await;
    assert_eq!(private.status, StatusCode::OK, "{}", private.text);
    app.admin_client
        .execute(
            "INSERT INTO public.guided_auto_owner (body) VALUES ('old record')",
            &[],
        )
        .await
        .unwrap();
    let automatic = send(
        &app,
        Method::POST,
        "/admin/api/tables/guided_auto_owner/policies",
        &cookie,
        json!({
            "policy": definition, "prepare_access": true, "owner_column": "user_id"
        }),
    )
    .await;
    assert_eq!(automatic.status, StatusCode::OK, "{}", automatic.text);
    let unclaimed: Option<uuid::Uuid> = app
        .admin_client
        .query_one(
            "SELECT user_id FROM public.guided_auto_owner WHERE body = 'old record'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(unclaimed.is_none());
    let added = app
        .raw(
            Method::POST,
            "/rest/v1/guided_auto_owner",
            &[
                ("authorization", bearer.as_str()),
                ("content-type", "application/json"),
            ],
            json!({ "body": "new record" }).to_string(),
        )
        .await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.text);
    let visible = app
        .raw(
            Method::GET,
            "/rest/v1/guided_auto_owner",
            &[("authorization", bearer.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(visible.status, StatusCode::OK, "{}", visible.text);
    assert_eq!(visible.body.as_array().unwrap().len(), 1);
}
