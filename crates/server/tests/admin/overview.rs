//! Panel overview integration scenarios.
use crate::common::panel::{get, login, send};
use crate::common::*;
use axum::http::Method;
use serde_json::json;

#[tokio::test]
async fn owner_views_warn_while_invoker_views_apply_the_callers_rls() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    app.admin_client.batch_execute(
        "GRANT SELECT ON public.todos TO anon;
         CREATE VIEW public.owner_todos AS SELECT * FROM public.todos;
         CREATE VIEW public.caller_todos WITH (security_invoker = true) AS SELECT * FROM public.todos;
         CREATE VIEW public.private_todos AS SELECT * FROM public.todos;
         CREATE MATERIALIZED VIEW public.snapshot_todos AS SELECT * FROM public.todos;
         GRANT SELECT ON public.owner_todos, public.caller_todos, public.snapshot_todos TO anon, authenticated;"
    ).await.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let overview = loop {
        let value = get(&app, "/admin/api/overview", &cookie).await.body;
        if value["tables"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "snapshot_todos")
        {
            break value;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "view metadata did not reload"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };
    assert_eq!(
        overview["exposed_without_rls"],
        json!(["owner_todos", "products", "snapshot_todos"])
    );
    for (name, expected) in [
        ("owner_todos", "danger"),
        ("caller_todos", "view"),
        ("private_todos", "view"),
        ("snapshot_todos", "danger"),
    ] {
        let row = overview["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap();
        assert_eq!(row["rls"]["state"], expected, "{name}");
    }
    let policies = get(&app, "/admin/api/policies", &cookie).await.body;
    assert_eq!(
        policies["exposed_without_rls"],
        overview["exposed_without_rls"]
    );
    // Policy editing stays on tables; views have their own remediation.
    assert!(
        !policies["tables"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "owner_todos")
    );
    let (_, base) = app.get("/rest/v1/todos", None).await;
    let (_, invoker) = app.get("/rest/v1/caller_todos", None).await;
    let (_, owner) = app.get("/rest/v1/owner_todos", None).await;
    assert_eq!(base, json!([]));
    assert_eq!(invoker, base);
    assert!(!owner.as_array().unwrap().is_empty());
    app.admin_client
        .batch_execute("ALTER VIEW public.owner_todos SET (security_invoker = true)")
        .await
        .unwrap();
    loop {
        let value = get(&app, "/admin/api/overview", &cookie).await.body;
        if !value["exposed_without_rls"]
            .as_array()
            .unwrap()
            .contains(&json!("owner_todos"))
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "fixed view still warns"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn warning_for_a_table_without_rls() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let overview = get(&app, "/admin/api/overview", &cookie).await.body;
    assert_eq!(overview["exposed_without_rls"], json!(["products"]));
    let table = |name: &str| {
        overview["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .cloned()
            .unwrap()
    };
    assert_eq!(table("products")["rls"]["state"], "danger");
    assert_eq!(table("todos")["rls"]["state"], "ok");
    assert_eq!(table("todos")["rls"]["label"], "RLS · 4 policies");
    // secrets has no GRANT for anon/authenticated: it is not "exposed".
    assert_eq!(table("secrets")["rls"]["state"], "none");
    assert_eq!(table("products")["grants"]["anon"], json!(["SELECT"]));
    assert_eq!(table("products")["rows"], 4);
    assert_eq!(table("products")["rows_exact"], true);
    assert!(overview["counts"]["functions"].as_u64().unwrap() >= 5);

    let policies = get(&app, "/admin/api/policies", &cookie).await.body;
    let todos = policies["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "todos")
        .unwrap();
    let select = todos["policies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "todos_owner_select")
        .unwrap();
    assert_eq!(select["using"], "(user_id = auth.uid())");
    assert_eq!(select["command"], "SELECT");
    assert_eq!(select["roles"], json!(["authenticated"]));
    assert!(
        policies["anon_functions"]
            .as_array()
            .unwrap()
            .contains(&json!("add"))
    );
    assert!(
        !policies["anon_functions"]
            .as_array()
            .unwrap()
            .contains(&json!("service_only"))
    );

    // Once the problem is fixed, the warning goes away (the catalog reloads by itself).
    app.admin_client
        .batch_execute("ALTER TABLE public.products ENABLE ROW LEVEL SECURITY")
        .await
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let overview = get(&app, "/admin/api/overview", &cookie).await.body;
        if overview["exposed_without_rls"] == json!([]) {
            let products = overview["tables"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["name"] == "products")
                .unwrap()
                .clone();
            assert_eq!(products["rls"]["state"], "warn");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "warning did not go away"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn counts_users_who_signed_in_through_the_api() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    let counts =
        || async { get(&app, "/admin/api/overview", &cookie).await.body["counts"].clone() };
    let before = counts().await;

    // Created in the panel: a user, but nobody signed in through an app yet.
    send(
        &app,
        Method::POST,
        "/admin/api/users",
        &cookie,
        json!({ "email": "staff@example.com", "password": "strong-password-123" }),
    )
    .await;
    let after_panel = counts().await;
    assert_eq!(after_panel["users"], before["users"].as_i64().unwrap() + 1);
    assert_eq!(after_panel["signed_in_users"], before["signed_in_users"]);

    // Signing up through the API opens a session: the app is connected.
    app.post(
        "/auth/v1/signup",
        None,
        json!({ "email": "app@example.com", "password": "strong-password-123" }),
    )
    .await;
    let after_app = counts().await;
    assert_eq!(
        after_app["signed_in_users"],
        before["signed_in_users"].as_i64().unwrap() + 1
    );
}
