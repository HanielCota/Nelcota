//! Panel overview integration scenarios.
use crate::common::panel::{get, login};
use crate::common::*;
use serde_json::json;

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
