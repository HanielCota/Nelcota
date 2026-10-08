//! Panel sql integration scenarios.
use crate::common::panel::{JSON, get, login, sql};
use crate::common::*;
use axum::http::{Method, StatusCode};
use serde_json::json;

#[tokio::test]
async fn sql_editor_is_isolated_and_has_readable_errors() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let out = sql(
        &app,
        &cookie,
        "select 1 as one, null as nothing; select 'b' as two",
    )
    .await;
    let results = out["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["columns"], json!(["one", "nothing"]));
    assert_eq!(results[0]["rows"], json!([["1", null]]));
    assert_eq!(results[1]["rows"], json!([["b"]]));

    let out = sql(&app, &cookie, "select * from table_that_does_not_exist").await;
    assert_eq!(out["error"]["code"], "42P01");
    assert!(out["error"]["position"].is_number());

    // An open transaction and SET ROLE do not leak into the next run.
    sql(&app, &cookie, "BEGIN; SET ROLE anon;").await;
    let out = sql(&app, &cookie, "select current_user::text").await;
    assert_eq!(out["results"][0]["rows"], json!([["postgres"]]));

    // CSRF: a different origin is refused.
    for extra in [
        [
            ("origin", "https://malicious.example"),
            ("host", "localhost"),
        ],
        [("sec-fetch-site", "cross-site"), ("host", "localhost")],
    ] {
        let mut headers = vec![("cookie", cookie.as_str()), JSON];
        headers.extend(extra);
        let reply = app
            .raw(
                Method::POST,
                "/admin/api/sql",
                &headers,
                json!({ "sql": "drop table public.todos" }).to_string(),
            )
            .await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN);
    }
    let out = sql(&app, &cookie, "select count(*) from public.todos").await;
    assert_eq!(out["results"][0]["rows"], json!([["2"]]));

    // Schema for the editor's autocomplete.
    let schema = get(&app, "/admin/api/schema", &cookie).await.body;
    assert!(
        schema["tables"]["products"]
            .as_array()
            .unwrap()
            .contains(&json!("price"))
    );
    assert!(
        schema["tables"]["auth.users"]
            .as_array()
            .unwrap()
            .contains(&json!("email"))
    );
}
