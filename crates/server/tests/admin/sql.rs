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

async fn run_as(app: &TestApp, cookie: &str, query: &str, run_as: serde_json::Value) -> Reply {
    crate::common::panel::send(
        app,
        Method::POST,
        "/admin/api/sql",
        cookie,
        json!({ "sql": query, "run_as": run_as }),
    )
    .await
}

fn first_cell(reply: &Reply) -> serde_json::Value {
    reply.body["results"][0]["rows"][0][0].clone()
}

#[tokio::test]
async fn sql_runs_as_a_visitor_or_a_signed_in_user_under_the_access_rules() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    let signup = app
        .post(
            "/auth/v1/signup",
            None,
            json!({ "email": "tester@example.com", "password": "strong-password-123" }),
        )
        .await;
    let id = signup.body["user"]["id"].as_str().unwrap().to_owned();
    app.admin_client
        .execute(
            "INSERT INTO public.todos (user_id, title) VALUES ($1::text::uuid, 'task of the tester')",
            &[&id],
        )
        .await
        .unwrap();
    let count = "select count(*) from public.todos";

    // The owner sees every row (no RLS); so does an explicit owner run.
    let owner = run_as(&app, &cookie, count, json!({ "role": "owner" })).await;
    assert_eq!(first_cell(&owner), "3", "{}", owner.text);

    // A signed-in user sees only their rows, and auth.uid() is theirs.
    let user = json!({ "role": "authenticated", "user_id": id });
    let mine = run_as(&app, &cookie, count, user.clone()).await;
    assert_eq!(first_cell(&mine), "1", "{}", mine.text);
    let who = run_as(
        &app,
        &cookie,
        "select auth.uid()::text, auth.jwt() ->> 'email'",
        user.clone(),
    )
    .await;
    assert_eq!(
        who.body["results"][0]["rows"][0],
        json!([id, "tester@example.com"])
    );
    // Writes follow the policies too: a row for someone else is refused.
    let foreign = run_as(
        &app,
        &cookie,
        "insert into public.todos (user_id, title) values (gen_random_uuid(), 'not mine')",
        user,
    )
    .await;
    assert_eq!(foreign.body["error"]["code"], "42501", "{}", foreign.text);

    // A visitor has no grant on todos at all.
    let anon = run_as(&app, &cookie, count, json!({ "role": "anon" })).await;
    assert_eq!(anon.body["error"]["code"], "42501", "{}", anon.text);

    for user_id in ["054f8cd2-decb-4c78-91a1-f351bd8f5b92", "not-a-uuid"] {
        let missing = run_as(
            &app,
            &cookie,
            count,
            json!({ "role": "authenticated", "user_id": user_id }),
        )
        .await;
        assert_eq!(missing.status, StatusCode::NOT_FOUND, "{user_id}");
        assert_eq!(missing.body["code"], "user_not_found");
    }
}
