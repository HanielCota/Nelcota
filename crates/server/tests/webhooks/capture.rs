use crate::{
    common::{TestApp, user_token},
    support,
};
use axum::http::{Method, StatusCode};
use nelcota_core::db;
use nelcota_webhooks::{management, models::HistoryQuery};
use serde_json::{Value, json};

#[tokio::test]
async fn capture_is_transactional_covers_every_verb_and_preserves_rls() {
    let app = TestApp::spawn().await;
    let first = support::hook(&app, "https://example.com/first", 6).await;
    let mut input = support::input("https://example.com/second");
    input.events = vec!["INSERT".into()];
    management::create(
        &db::admin_pool(&app.admin, 2),
        "public",
        input,
        Default::default(),
    )
    .await
    .unwrap();
    app.admin_client.batch_execute("BEGIN; INSERT INTO public.todos (user_id,title) VALUES (gen_random_uuid(),'rolled back'); ROLLBACK;").await.unwrap();
    assert_eq!(support::count(&app).await, 0);
    let token = user_token(app.user_a);
    let inserted = app
        .request(
            Method::POST,
            "/rest/v1/todos",
            Some(&token),
            Some(json!({"title":"New task"})),
        )
        .await;
    assert_eq!(inserted.status, StatusCode::CREATED, "{}", inserted.text);
    assert_eq!(support::count(&app).await, 2);
    let same: i64 = app
        .admin_client
        .query_one(
            "SELECT count(DISTINCT event_id) FROM nelcota.webhook_deliveries",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(same, 1);
    let id: i64 = app
        .admin_client
        .query_one("SELECT id FROM public.todos WHERE title='New task'", &[])
        .await
        .unwrap()
        .get(0);
    let hidden = app
        .request(
            Method::PATCH,
            &format!("/rest/v1/todos?id=eq.{id}"),
            Some(&user_token(app.user_b)),
            Some(json!({"title":"Not yours"})),
        )
        .await;
    assert_eq!(hidden.status, StatusCode::NO_CONTENT);
    assert_eq!(support::count(&app).await, 2);
    let updated = app
        .request(
            Method::PATCH,
            &format!("/rest/v1/todos?id=eq.{id}"),
            Some(&token),
            Some(json!({"title":"Changed"})),
        )
        .await;
    assert_eq!(updated.status, StatusCode::NO_CONTENT);
    assert_eq!(support::count(&app).await, 3);
    let deleted = app
        .request(
            Method::DELETE,
            &format!("/rest/v1/todos?id=eq.{id}"),
            Some(&token),
            None,
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_eq!(support::count(&app).await, 4);
    let pool = db::admin_pool(&app.admin, 2);
    let history = management::history(
        &pool,
        HistoryQuery {
            endpoint: Some(first.endpoint.id),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(history.deliveries.len(), 3);
    let payload: Value = serde_json::from_str(
        &management::detail(&pool, &history.deliveries[0].id)
            .await
            .unwrap()
            .payload,
    )
    .unwrap();
    assert_eq!(payload["type"], "DELETE");
    assert!(payload["record"].is_null());
    assert_eq!(payload["old_record"]["title"], "Changed");
    let update: Value = serde_json::from_str(
        &management::detail(&pool, &history.deliveries[1].id)
            .await
            .unwrap()
            .payload,
    )
    .unwrap();
    assert_eq!(update["old_record"]["title"], "New task");
    assert_eq!(update["record"]["title"], "Changed");
    for role in ["anon", "authenticated", "service_role"] {
        let denied: bool=app.admin_client.query_one("SELECT NOT has_table_privilege($1::text,'nelcota.webhook_endpoints','SELECT') AND NOT has_table_privilege($1::text,'nelcota.webhook_deliveries','SELECT') AND NOT has_function_privilege($1::text,'nelcota.capture_webhook_event()','EXECUTE')", &[&role]).await.unwrap().get(0);
        assert!(denied, "{role}");
    }
    let usage = app
        .admin_client
        .query_one(
            "SELECT outstanding,retained FROM nelcota.webhook_queue_usage",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(usage.get::<_, i64>(0), 4);
    assert_eq!(usage.get::<_, i64>(1), 4);
}

#[tokio::test]
async fn pause_rename_delete_and_large_payload_have_explicit_outcomes() {
    let app = TestApp::spawn().await;
    let hook = support::hook(&app, "https://example.com/hook", 6).await;
    let pool = db::admin_pool(&app.admin, 2);
    let mut paused = support::input(&hook.endpoint.url);
    paused.enabled = false;
    management::update(&pool, &hook.endpoint.id, paused, Default::default())
        .await
        .unwrap();
    support::insert(&app, "not captured").await;
    assert_eq!(support::count(&app).await, 0);
    management::update(
        &pool,
        &hook.endpoint.id,
        support::input(&hook.endpoint.url),
        Default::default(),
    )
    .await
    .unwrap();
    app.admin_client
        .batch_execute("ALTER TABLE public.todos RENAME TO tasks")
        .await
        .unwrap();
    app.admin_client
        .execute(
            "INSERT INTO public.tasks (user_id,title) VALUES ($1,'renamed')",
            &[&app.user_a],
        )
        .await
        .unwrap();
    assert_eq!(management::list(&pool).await.unwrap()[0].table, "tasks");
    assert_eq!(support::count(&app).await, 1);
    let large = "a".repeat(300_000);
    app.admin_client
        .execute(
            "INSERT INTO public.tasks (user_id,title) VALUES ($1,$2)",
            &[&app.user_a, &large],
        )
        .await
        .unwrap();
    let id = support::latest(&app).await;
    let detail = management::detail(&pool, &id).await.unwrap();
    assert_eq!(detail.delivery.status, "failed");
    assert_eq!(
        detail.delivery.last_error.as_deref(),
        Some("payload_too_large")
    );
    assert!(detail.payload.len() < 1024);
    assert_eq!(detail.delivery.attempts, 0);
    assert!(management::replay(&pool, &id).await.is_err());
    management::delete(&pool, &hook.endpoint.id).await.unwrap();
    let active: i64 = app
        .admin_client
        .query_one("SELECT outstanding FROM nelcota.webhook_queue_usage", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(active, 0);
    let original = app
        .admin_client
        .query_one(
            "SELECT status FROM nelcota.webhook_deliveries ORDER BY id LIMIT 1",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(original.get::<_, &str>(0), "cancelled");
    assert_eq!(management::clear_history(&pool).await.unwrap(), 2);
    assert_eq!(
        app.admin_client
            .query_one("SELECT retained FROM nelcota.webhook_queue_usage", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test]
async fn keyset_history_and_capacity_protect_data_and_queue() {
    let app = TestApp::spawn().await;
    support::hook(&app, "https://example.com/hook", 6).await;
    let pool = db::admin_pool(&app.admin, 2);
    app.admin_client.execute("INSERT INTO public.todos (user_id,title) SELECT $1,'page '||n FROM generate_series(1,31) n", &[&app.user_a]).await.unwrap();
    let first = management::history(&pool, Default::default())
        .await
        .unwrap();
    assert_eq!(first.deliveries.len(), 25);
    assert!(first.next_cursor.is_some());
    let second = management::history(
        &pool,
        HistoryQuery {
            before: first.next_cursor,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(second.deliveries.len(), 6);
    assert!(second.next_cursor.is_none());
    assert!(
        first
            .deliveries
            .iter()
            .all(|d| second.deliveries.iter().all(|other| other.id != d.id))
    );
    assert_eq!(
        management::clear_history(&pool).await.unwrap(),
        0,
        "active deliveries cannot be cleared"
    );
    app.admin_client
        .execute(
            "UPDATE nelcota.webhook_queue_usage SET outstanding=10000 WHERE singleton",
            &[],
        )
        .await
        .unwrap();
    let failed = app
        .admin_client
        .execute(
            "INSERT INTO public.todos (user_id,title) VALUES ($1,'must roll back')",
            &[&app.user_a],
        )
        .await
        .unwrap_err();
    assert_eq!(failed.code().unwrap().code(), "53300");
    assert_eq!(support::count(&app).await, 31);
    assert_eq!(
        app.admin_client
            .query_one(
                "SELECT count(*) FROM public.todos WHERE title='must roll back'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test]
async fn deletion_does_not_deadlock_a_writer_and_truncation_resets_capacity() {
    use std::time::Duration;
    let app = TestApp::spawn().await;
    let hook = support::hook(&app, "https://example.com/hook", 6).await;
    support::insert(&app, "existing pending delivery").await;
    let (mut writer, connection) = app.admin.connect(tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(connection);
    let tx = writer.transaction().await.unwrap();
    tx.batch_execute("LOCK TABLE public.todos IN ROW EXCLUSIVE MODE")
        .await
        .unwrap();
    let pool = db::admin_pool(&app.admin, 2);
    let id = hook.endpoint.id.clone();
    let deleting = tokio::spawn(async move { management::delete(&pool, &id).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let waiting: bool = app.admin_client.query_one("SELECT EXISTS (SELECT FROM pg_locks WHERE relation='public.todos'::regclass AND NOT granted)", &[]).await.unwrap().get(0);
            if waiting { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    tokio::time::timeout(
        Duration::from_secs(2),
        tx.execute(
            "INSERT INTO public.todos (user_id,title) VALUES ($1,'writer while deleting')",
            &[&app.user_a],
        ),
    )
    .await
    .unwrap()
    .unwrap();
    tx.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), deleting)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(support::count(&app).await, 2);
    let statuses: Vec<String> = app
        .admin_client
        .query("SELECT status FROM nelcota.webhook_deliveries", &[])
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert!(statuses.iter().all(|s| s == "cancelled"));
    app.admin_client
        .batch_execute("TRUNCATE nelcota.webhook_deliveries CASCADE")
        .await
        .unwrap();
    let usage = app
        .admin_client
        .query_one(
            "SELECT outstanding,retained FROM nelcota.webhook_queue_usage",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(usage.get::<_, i64>(0), 0);
    assert_eq!(usage.get::<_, i64>(1), 0);
}
