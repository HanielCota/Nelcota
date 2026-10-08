//! Panel migrations integration scenarios.
use crate::common::panel::{get, login, send};
use crate::common::*;
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};

fn simple_table(name: &str) -> Value {
    json!({ "name": name, "columns": [
        { "name": "id", "data_type": "bigint", "primary_key": true, "identity": true },
        { "name": "body", "data_type": "text" },
    ] })
}

/// Runs migrations as `nelcota migrate` does (refinery, `abort_divergent`).
async fn refinery_migrate(app: &TestApp, files: &[(&str, &str)]) -> Result<usize, String> {
    let (mut client, connection) = app.admin.connect(tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(connection);
    let migrations: Vec<_> = files
        .iter()
        .map(|(stem, sql)| refinery::Migration::unapplied(stem, sql).unwrap())
        .collect();
    let mut runner = refinery::Runner::new(&migrations).set_abort_divergent(true);
    runner.set_migration_table_name("nelcota.user_migrations");
    runner
        .run_async(&mut client)
        .await
        .map(|report| report.applied_migrations().len())
        .map_err(|e| e.to_string())
}

#[tokio::test]
async fn panel_changes_become_a_migration_recognised_by_migrate() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    let create = |table: Value| send(&app, Method::POST, "/admin/api/tables", &cookie, table);
    let export = |name: &str| {
        send(
            &app,
            Method::POST,
            "/admin/api/migrations",
            &cookie,
            json!({ "name": name }),
        )
    };

    // A preview and DDL refused by the database do not count as changes.
    create(json!({ "table": simple_table("orders"), "preview": true })).await;
    let bad = create(
        json!({ "table": { "name": "x", "columns": [{ "name": "a", "data_type": "money" }] } }),
    )
    .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let list = get(&app, "/admin/api/migrations", &cookie).await.body;
    assert_eq!(list["pending"], json!([]));
    assert_eq!(list["next_version"], 1);
    assert!(list["folder"].is_null());

    for name in ["orders", "items"] {
        let reply = create(json!({ "table": simple_table(name) })).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    }
    let list = get(&app, "/admin/api/migrations", &cookie).await.body;
    let pending = list["pending"].as_array().unwrap();
    assert_eq!(pending.len(), 2);
    // Kind and target let the panel describe the change in its own language.
    assert_eq!(
        (&pending[0]["kind"], &pending[0]["target"]),
        (&json!("table_created"), &json!("orders"))
    );
    // ISO 8601 with the full offset ("+00:00"), which the browser understands.
    let at = pending[0]["applied_at"].as_str().unwrap();
    let offset = &at[at.len() - 6..];
    assert!(
        at.contains('T')
            && (offset.starts_with('+') || offset.starts_with('-'))
            && offset.as_bytes()[3] == b':',
        "{at}"
    );
    assert!(
        pending[0]["statements"][0]
            .as_str()
            .unwrap()
            .starts_with("CREATE TABLE \"public\".\"orders\"")
    );

    assert_eq!(
        export("Create Orders").await.status,
        StatusCode::BAD_REQUEST
    );
    let reply = export("create_orders").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.body["version"], 1);
    assert_eq!(reply.body["filename"], "V1__create_orders.sql");
    let sql = reply.body["sql"].as_str().unwrap().to_owned();
    assert!(sql.contains("CREATE TABLE \"public\".\"orders\""));
    assert!(sql.contains("CREATE TABLE \"public\".\"items\""));

    let list = get(&app, "/admin/api/migrations", &cookie).await.body;
    assert_eq!(list["pending"], json!([]));
    let first = &list["migrations"][0];
    assert_eq!(
        (&first["version"], &first["name"], &first["from_panel"]),
        (&json!(1), &json!("create_orders"), &json!(true))
    );
    assert!(first["applied_on"].is_string());
    let again = export("again").await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.body["code"], "no_pending_changes");

    // The file can be downloaded again, unchanged.
    let file = get(&app, "/admin/api/migrations/1/file", &cookie).await;
    assert_eq!(file.status, StatusCode::OK);
    assert_eq!(file.text, sql);
    assert!(
        file.headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("V1__create_orders.sql")
    );

    // migrate recognises the file as already applied (nothing runs twice)...
    assert_eq!(
        refinery_migrate(&app, &[("V1__create_orders", &sql)]).await,
        Ok(0)
    );
    // ...and refuses the edited file.
    let edited = format!("{sql}-- edited\n");
    assert!(
        refinery_migrate(&app, &[("V1__create_orders", &edited)])
            .await
            .is_err()
    );

    // After a hand-written migration (V2), the next generated one is V3.
    let v2 = "CREATE INDEX orders_body_idx ON public.orders (body);";
    assert_eq!(
        refinery_migrate(&app, &[("V1__create_orders", &sql), ("V2__index", v2)]).await,
        Ok(1)
    );
    create(json!({ "table": simple_table("customers") })).await;
    let reply = export("customers").await;
    assert_eq!(reply.body["version"], 3, "{}", reply.text);
}

#[tokio::test]
async fn concurrent_migration_exports_record_pending_changes_once() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    let created = send(
        &app,
        Method::POST,
        "/admin/api/tables",
        &cookie,
        json!({ "table": simple_table("concurrent_export") }),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK);
    let (first, second) = tokio::join!(
        send(
            &app,
            Method::POST,
            "/admin/api/migrations",
            &cookie,
            json!({ "name": "first_export" })
        ),
        send(
            &app,
            Method::POST,
            "/admin/api/migrations",
            &cookie,
            json!({ "name": "second_export" })
        )
    );
    let (winner, loser) = if first.status == StatusCode::OK {
        (first, second)
    } else {
        (second, first)
    };
    assert_eq!(winner.status, StatusCode::OK);
    assert_eq!(loser.status, StatusCode::CONFLICT);
    assert_eq!(loser.body["code"], "no_pending_changes");
    let listed = get(&app, "/admin/api/migrations", &cookie).await;
    assert_eq!(listed.body["pending"], json!([]));
    assert_eq!(listed.body["migrations"].as_array().unwrap().len(), 1);
    let filename = winner.body["filename"]
        .as_str()
        .unwrap()
        .strip_suffix(".sql")
        .unwrap();
    assert_eq!(
        refinery_migrate(&app, &[(filename, winner.body["sql"].as_str().unwrap())]).await,
        Ok(0)
    );
}

#[tokio::test]
async fn generated_migration_never_collides_with_an_unapplied_file() {
    let dir = std::env::temp_dir().join(format!("nelcota-migrations-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("V1__base.sql"), "select 1;").unwrap();
    std::fs::write(dir.join("V4__not_applied_yet.sql"), "select 1;").unwrap();
    std::fs::write(dir.join("README.md"), "not a migration").unwrap();
    let app = TestApp::spawn_with(Options {
        migrations_dir: Some(dir.clone()),
        ..Options::default()
    })
    .await;
    let cookie = login(&app).await;

    send(
        &app,
        Method::POST,
        "/admin/api/tables",
        &cookie,
        json!({ "table": simple_table("orders") }),
    )
    .await;
    let list = get(&app, "/admin/api/migrations", &cookie).await.body;
    assert_eq!(list["next_version"], 5);
    assert_eq!(list["folder"], dir.display().to_string());
    let v4 = list["migrations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["version"] == 4)
        .unwrap();
    assert_eq!(v4["in_folder"], true);
    assert!(v4["applied_on"].is_null());

    let reply = send(
        &app,
        Method::POST,
        "/admin/api/migrations",
        &cookie,
        json!({ "name": "orders" }),
    )
    .await;
    assert_eq!(reply.body["filename"], "V5__orders.sql", "{}", reply.text);
    std::fs::remove_dir_all(&dir).ok();
}
