//! Regression coverage for the follow-up performance and CSV audit.
mod common;

use axum::http::StatusCode;
use common::{TestApp, panel};
use nelcota_core::{Claims, db::begin_request};
use serde_json::{Map, Value, json};

#[tokio::test]
async fn csv_export_neutralizes_formulas_from_public_input_and_preserves_json() {
    let app = TestApp::spawn().await;
    app.admin_client
        .batch_execute(
            "CREATE TABLE public.audit_csv(id integer PRIMARY KEY, value text, amount numeric);
         GRANT INSERT ON public.audit_csv TO anon;",
        )
        .await
        .unwrap();
    app.catalog.reload(&app.pool).await.unwrap();
    let response = app
        .post(
            "/rest/v1/audit_csv",
            None,
            json!([
                {"id":1,"value":"=1+1","amount":-12.50},
                {"id":2,"value":"+1+1"}, {"id":3,"value":"@SUM(1,1)"},
                {"id":4,"value":" =1+1"}, {"id":5,"value":"\t=1+1"},
                {"id":6,"value":"=HYPERLINK(\"https://example.com\",\"open\")"},
            ]),
        )
        .await;
    assert_eq!(response.status, StatusCode::CREATED);
    let cookie = panel::login(&app).await;
    let csv = panel::get(
        &app,
        "/admin/api/tables/audit_csv/export?format=csv&sort=id",
        &cookie,
    )
    .await;
    assert_eq!(csv.status, StatusCode::OK);
    assert!(
        csv.text
            .starts_with("\u{feff}id,value,amount\r\n1,\"\t=1+1\",-12.5\r\n"),
        "{}",
        csv.text
    );
    assert!(csv.text.contains("2,\"\t+1+1\",\r\n"));
    assert!(csv.text.contains("3,\"\t@SUM(1,1)\",\r\n"));
    assert!(csv.text.contains("4,\"\t =1+1\",\r\n"));
    assert!(csv.text.contains("5,\"\t\t=1+1\",\r\n"));
    assert!(
        csv.text
            .contains("6,\"\t=HYPERLINK(\"\"https://example.com\"\",\"\"open\"\")\",\r\n")
    );
    let raw = panel::get(
        &app,
        "/admin/api/tables/audit_csv/export?format=json&sort=id",
        &cookie,
    )
    .await;
    assert_eq!(raw.status, StatusCode::OK);
    let rows: serde_json::Value = serde_json::from_str(&raw.text).unwrap();
    assert_eq!(rows[0]["value"], "=1+1");
    assert_eq!(rows[4]["value"], "\t=1+1");
}

#[tokio::test]
async fn storage_listing_does_not_spill_all_metadata_before_limiting_the_page() {
    let app = TestApp::spawn().await;
    app.admin_client
        .batch_execute(
            "INSERT INTO storage.buckets(id) VALUES('audit-scale');
         INSERT INTO storage.objects(bucket_id,name,version,size,mime_type,etag,metadata)
         SELECT 'audit-scale',lpad(n::text,8,'0') || '.txt',gen_random_uuid(),0,
                'text/plain','',jsonb_build_object('payload',repeat('x',2048))
         FROM generate_series(1,10000) n; ANALYZE storage.objects;",
        )
        .await
        .unwrap();
    let claims = Claims::from_payload(json!({"role":"service_role"})).unwrap();
    let mut client = app.pool.get().await.unwrap();
    let tx = begin_request(&mut client, &claims).await.unwrap();
    tx.batch_execute("SET LOCAL work_mem = '1MB'")
        .await
        .unwrap();
    let sql = include_str!("../../storage/src/list.sql");
    let plan: serde_json::Value = tx
        .query_one(
            &format!("EXPLAIN (ANALYZE,BUFFERS,FORMAT JSON) {sql}"),
            &[&"audit-scale", &"%", &0_i32, &10_i64, &0_i64],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        plan[0]["Plan"]["Temp Written Blocks"], 0,
        "a small flat-file page must not materialize every object's metadata: {plan}"
    );
    tx.commit().await.unwrap();
    drop(client);
    let page = app
        .storage
        .list(&claims, "audit-scale", "", Some(10), 3)
        .await
        .unwrap();
    assert!(page.folders.is_empty());
    assert_eq!(page.objects.len(), 10);
    assert_eq!(page.objects[0].name, "00000004.txt");
    assert_eq!(
        page.objects[0].metadata["payload"].as_str().unwrap().len(),
        2048
    );
}

#[tokio::test]
async fn wide_batches_preserve_values_defaults_and_reject_invalid_rows_atomically() {
    let app = TestApp::spawn().await;
    let columns = (0..1600)
        .rev()
        .map(|n| format!("c{n:04} integer{}", if n == 0 { " DEFAULT 42" } else { "" }))
        .collect::<Vec<_>>()
        .join(",");
    app.admin_client
        .batch_execute(&format!(
            "CREATE TABLE public.audit_wide({columns}); GRANT INSERT ON public.audit_wide TO anon"
        ))
        .await
        .unwrap();
    app.catalog.reload(&app.pool).await.unwrap();
    let template = (1500..1600)
        .map(|n| (format!("c{n:04}"), json!(1)))
        .collect::<Map<_, _>>();
    let rows = (0..1000)
        .map(|n| {
            let mut row = template.clone();
            row.insert("c1500".into(), json!(n));
            Value::Object(row)
        })
        .collect::<Vec<_>>();
    assert!(serde_json::to_vec(&rows).unwrap().len() < 2 * 1024 * 1024);
    let inserted = app.post("/rest/v1/audit_wide", None, json!(rows)).await;
    assert_eq!(inserted.status, StatusCode::CREATED, "{}", inserted.text);
    let stored = app.admin_client.query_one(
        "SELECT count(*), min(c1500), max(c1500), bool_and(c0000=42), bool_and(c1599=1), bool_and(c1499 IS NULL) FROM public.audit_wide", &[]
    ).await.unwrap();
    assert_eq!(stored.get::<_, i64>(0), 1000);
    assert_eq!(stored.get::<_, i32>(1), 0);
    assert_eq!(stored.get::<_, i32>(2), 999);
    assert!(stored.get::<_, bool>(3));
    assert!(stored.get::<_, bool>(4));
    assert!(stored.get::<_, bool>(5));
    for body in [
        json!([{"c1500":1001},{"missing":1}]),
        json!([{"c1500":1001},false]),
    ] {
        let rejected = app.post("/rest/v1/audit_wide", None, body).await;
        assert_eq!(
            rejected.status,
            StatusCode::BAD_REQUEST,
            "{}",
            rejected.text
        );
    }
    app.admin_client
        .batch_execute("REVOKE INSERT ON public.audit_wide FROM anon")
        .await
        .unwrap();
    let denied = app
        .post("/rest/v1/audit_wide", None, json!([{"c1500":1001}]))
        .await;
    assert_eq!(denied.status, StatusCode::UNAUTHORIZED, "{}", denied.text);
    let count: i64 = app
        .admin_client
        .query_one("SELECT count(*) FROM public.audit_wide", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count, 1000,
        "invalid or unauthorized batches cannot publish rows"
    );
}
