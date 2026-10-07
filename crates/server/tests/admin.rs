//! Admin panel tests: separate login, protected JSON API, CSRF, the
//! no-RLS table warning, SQL editor, table editing, users, policies and the
//! embedded SPA.

mod common;

use axum::http::{Method, StatusCode, header};
use common::*;
use serde_json::{Value, json};

const JSON: (&str, &str) = ("content-type", "application/json");

async fn login(app: &TestApp) -> String {
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

async fn get(app: &TestApp, path: &str, cookie: &str) -> Reply {
    app.raw(Method::GET, path, &[("cookie", cookie)], String::new())
        .await
}

async fn send(app: &TestApp, method: Method, path: &str, cookie: &str, body: Value) -> Reply {
    app.raw(method, path, &[("cookie", cookie), JSON], body.to_string())
        .await
}

async fn sql(app: &TestApp, cookie: &str, query: &str) -> Value {
    send(
        app,
        Method::POST,
        "/admin/api/sql",
        cookie,
        json!({ "sql": query }),
    )
    .await
    .body
}

#[tokio::test]
async fn separate_login_and_protected_api() {
    let app = TestApp::spawn().await;

    for path in [
        "/admin/api/overview",
        "/admin/api/tables/products",
        "/admin/api/users",
    ] {
        let reply = app.raw(Method::GET, path, &[], String::new()).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{path}");
    }
    let reply = app
        .raw(
            Method::POST,
            "/admin/api/sql",
            &[JSON],
            json!({ "sql": "select 1" }).to_string(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);

    for (email, password) in [
        (ADMIN_EMAIL, "wrong-password"),
        ("someone@example.com", ADMIN_PASSWORD),
    ] {
        let reply = app
            .raw(
                Method::POST,
                "/admin/api/login",
                &[JSON],
                json!({ "email": email, "password": password }).to_string(),
            )
            .await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
        assert_eq!(reply.body["error"], "Invalid email or password.");
        // The panel translates by code (crates/admin/ui/src/lib/i18n/messages/errors.ts).
        assert_eq!(reply.body["code"], "invalid_credentials");
    }

    // A JWT (not even service_role) does not open the panel.
    let reply = app
        .raw(
            Method::GET,
            "/admin/api/overview",
            &[("authorization", &format!("Bearer {}", service_token()))],
            String::new(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);

    let cookie = login(&app).await;
    let reply = get(&app, "/admin/api/session", &cookie).await;
    assert_eq!(reply.body["email"], ADMIN_EMAIL);

    // Login the way a browser does it (same origin) passes; `Origin: null` does not.
    let browser = |origin: &'static str| {
        [
            JSON,
            ("host", "localhost"),
            ("origin", origin),
            ("sec-fetch-site", "same-origin"),
        ]
    };
    let credentials = json!({ "email": ADMIN_EMAIL, "password": ADMIN_PASSWORD }).to_string();
    let reply = app
        .raw(
            Method::POST,
            "/admin/api/login",
            &browser("https://localhost"),
            credentials.clone(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    let reply = app
        .raw(
            Method::POST,
            "/admin/api/login",
            &browser("null"),
            credentials,
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.body["code"], "origin_not_allowed");

    // Logout invalidates the session.
    let reply = send(&app, Method::POST, "/admin/api/logout", &cookie, json!({})).await;
    assert_eq!(reply.status, StatusCode::OK);
    let reply = get(&app, "/admin/api/session", &cookie).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.body["code"], "session_expired");
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

#[tokio::test]
async fn edit_tables_from_the_panel_with_exact_values() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let data = get(&app, "/admin/api/tables/products", &cookie).await.body;
    assert_eq!(data["table"]["primary_key"], json!(["id"]));
    assert_eq!(data["table"]["editable"], true);
    let price = data["table"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "price")
        .unwrap()
        .clone();
    assert_eq!(price["full_type"], "numeric(10,2)");
    assert!(
        data["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["name"] == "Notebook")
    );
    // numeric arrives as exact text (never through f64).
    assert!(
        data["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["price"] == "15.00")
    );
    assert_eq!(data["total"], 4);

    // Sort by column (validated against the catalog).
    let sorted = get(
        &app,
        "/admin/api/tables/products?sort=price&desc=true",
        &cookie,
    )
    .await
    .body;
    assert_eq!(sorted["rows"][0]["name"], "Backpack");
    let bad = get(&app, "/admin/api/tables/products?sort=price;drop", &cookie).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);

    // Insert: HTML-looking text is stored as text (escaping is Svelte's job,
    // which never uses {@html}); an omitted field uses the DEFAULT.
    let xss = "<script>alert(1)</script>";
    let reply = send(
        &app,
        Method::POST,
        "/admin/api/tables/products/rows",
        &cookie,
        json!({ "values": { "name": xss, "price": "9.90" } }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let row = app
        .admin_client
        .query_one(
            "SELECT id, stock, price::text FROM public.products WHERE name = $1",
            &[&xss],
        )
        .await
        .unwrap();
    let id: i32 = row.get(0);
    assert_eq!(row.get::<_, i32>(1), 0, "an omitted field uses the DEFAULT");
    assert_eq!(row.get::<_, String>(2), "9.90");

    let reply = send(
        &app,
        Method::PATCH,
        "/admin/api/tables/products/rows",
        &cookie,
        json!({ "pk": { "id": id.to_string() }, "values": { "name": "Pencil case", "stock": "5" } }),
    )
    .await;
    assert_eq!(reply.body["count"], 1, "{}", reply.text);

    // A database error comes back as a 400 with its message (and no code:
    // the panel shows Postgres' text as-is).
    let reply = send(
        &app,
        Method::PATCH,
        "/admin/api/tables/products/rows",
        &cookie,
        json!({ "pk": { "id": id.to_string() }, "values": { "price": "-1" } }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert!(reply.body["error"].as_str().unwrap().contains("check"));
    assert!(reply.body.get("code").is_none(), "{}", reply.text);

    // An unknown or generated column is refused.
    for values in [
        json!({ "\"; drop table products; --": "1" }),
        json!({ "slug": "x" }),
    ] {
        let reply = send(
            &app,
            Method::POST,
            "/admin/api/tables/products/rows",
            &cookie,
            json!({ "values": values }),
        )
        .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    }
    let reply = send(
        &app,
        Method::POST,
        "/admin/api/tables/products/rows",
        &cookie,
        json!({ "values": { "slug": "x" } }),
    )
    .await;
    assert_eq!(reply.body["code"], "unknown_or_generated_column");
    assert_eq!(reply.body["params"], json!({ "column": "slug" }));

    // Delete several rows in one transaction.
    let reply = send(
        &app,
        Method::DELETE,
        "/admin/api/tables/products/rows",
        &cookie,
        json!({ "pks": [{ "id": id.to_string() }, { "id": "3" }] }),
    )
    .await;
    assert_eq!(reply.body["count"], 2, "{}", reply.text);
    let count: i64 = app
        .admin_client
        .query_one("SELECT count(*) FROM public.products", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 3);

    let reply = get(&app, "/admin/api/tables/missing", &cookie).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.body["code"], "table_not_found");
    assert_eq!(reply.body["params"], json!({ "table": "missing" }));
}

#[tokio::test]
async fn users_and_sessions() {
    let app = TestApp::spawn().await;
    let session = app
        .post(
            "/auth/v1/signup",
            None,
            json!({ "email": "panel@example.com", "password": "strong-password-123" }),
        )
        .await
        .body;
    let user_id = session["user"]["id"].as_str().unwrap().to_owned();
    let cookie = login(&app).await;

    let users = get(&app, "/admin/api/users?q=panel", &cookie).await;
    assert_eq!(users.body["users"][0]["email"], "panel@example.com");
    assert_eq!(users.body["users"][0]["sessions"], 1);
    assert!(
        !users.text.contains("argon2id"),
        "the hash never leaves the database"
    );

    let reply = send(
        &app,
        Method::POST,
        &format!("/admin/api/users/{user_id}/revoke"),
        &cookie,
        json!({}),
    )
    .await;
    assert_eq!(reply.body["count"], 1);
    let refresh = app
        .post(
            "/auth/v1/token?grant_type=refresh_token",
            None,
            json!({ "refresh_token": session["refresh_token"] }),
        )
        .await;
    assert_eq!(refresh.status, StatusCode::BAD_REQUEST);

    let reply = send(
        &app,
        Method::DELETE,
        &format!("/admin/api/users/{user_id}"),
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    let users = get(&app, "/admin/api/users", &cookie).await;
    assert!(!users.text.contains("panel@example.com"));

    let reply = send(
        &app,
        Method::DELETE,
        "/admin/api/users/not-a-uuid",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn embedded_spa_with_security_headers() {
    let app = TestApp::spawn().await;

    let index = app.raw(Method::GET, "/admin/", &[], String::new()).await;
    assert_eq!(index.status, StatusCode::OK);
    assert!(index.text.contains("<div id=\"app\">"));
    let csp = index.headers[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap();
    assert!(csp.contains("script-src 'self';"), "{csp}");
    assert!(
        !csp.contains("script-src 'self' 'unsafe"),
        "scripts never inline"
    );
    assert_eq!(index.headers[header::X_FRAME_OPTIONS], "DENY");
    // Regression: with `no-referrer` the browser sends `Origin: null` on POSTs.
    assert_eq!(index.headers[header::REFERRER_POLICY], "same-origin");
    assert!(
        !index.text.contains("<script>"),
        "no inline script in index.html"
    );

    // Client routes return the same index.html.
    let deep = app
        .raw(Method::GET, "/admin/tables/anything", &[], String::new())
        .await;
    assert_eq!(deep.status, StatusCode::OK);
    assert_eq!(deep.text, index.text);

    // The referenced bundle exists, with an immutable cache.
    let script = index
        .text
        .split("src=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("index.html references the bundle");
    assert!(script.starts_with("/admin/assets/index-"), "{script}");
    let js = app.raw(Method::GET, script, &[], String::new()).await;
    assert_eq!(js.status, StatusCode::OK);
    assert!(
        js.headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/javascript")
    );
    assert!(
        js.headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable")
    );

    let reply = app
        .raw(
            Method::GET,
            "/admin/assets/../Cargo.toml",
            &[],
            String::new(),
        )
        .await;
    assert_ne!(reply.status, StatusCode::OK);
    let reply = app
        .raw(Method::GET, "/admin/api/missing", &[], String::new())
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.body["code"], "route_not_found");
}

/// Handoff token signed with the shared secret (as another panel on the host
/// would do).
fn handoff_token(audience: &str, email: &str, exp_offset: i64, secret: &str) -> String {
    let now = jsonwebtoken::get_current_timestamp() as i64;
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &json!({
            "iss": "nelcota-admin",
            "sub": email,
            "aud": audience,
            "iat": now,
            "exp": now + exp_offset,
            "jti": uuid::Uuid::new_v4().to_string(),
        }),
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

async fn redeem(app: &TestApp, token: &str) -> Reply {
    app.raw(
        Method::POST,
        "/admin/api/sso",
        &[JSON],
        json!({ "token": token }).to_string(),
    )
    .await
}

#[tokio::test]
async fn single_sign_on_between_host_projects() {
    let app = TestApp::spawn().await;

    // The login screen knows which project it is on.
    let whoami = app
        .raw(Method::GET, "/admin/api/whoami", &[], String::new())
        .await;
    assert_eq!(whoami.body, json!({ "project": "shop", "sso": true }));

    // Token issued by another panel on the host for "shop": becomes a session.
    let token = handoff_token("shop", ADMIN_EMAIL, 60, SSO_SECRET);
    let reply = redeem(&app, &token).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let cookie = reply.headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    assert_eq!(
        get(&app, "/admin/api/session", &cookie).await.body["project"],
        "shop"
    );

    // Single use.
    assert_eq!(redeem(&app, &token).await.status, StatusCode::UNAUTHORIZED);

    // Wrong target, wrong admin, expired or wrong secret: refused.
    for bad in [
        handoff_token("blog", ADMIN_EMAIL, 60, SSO_SECRET),
        handoff_token("shop", "intruder@example.com", 60, SSO_SECRET),
        handoff_token("shop", ADMIN_EMAIL, -120, SSO_SECRET),
        handoff_token(
            "shop",
            ADMIN_EMAIL,
            60,
            "some-other-secret-with-32-bytes!!!!",
        ),
    ] {
        assert_eq!(redeem(&app, &bad).await.status, StatusCode::UNAUTHORIZED);
    }

    // A handoff to another project needs a session and an existing project.
    let reply = app
        .raw(
            Method::POST,
            "/admin/api/sso/handoff",
            &[JSON],
            json!({ "project": "blog" }).to_string(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    let reply = send(
        &app,
        Method::POST,
        "/admin/api/sso/handoff",
        &cookie,
        json!({ "project": "nothing" }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);

    let reply = send(
        &app,
        Method::POST,
        "/admin/api/sso/handoff",
        &cookie,
        json!({ "project": "blog" }),
    )
    .await;
    let url = reply.body["url"].as_str().unwrap().to_owned();
    let (base, token) = url.split_once("/admin/#sso=").unwrap();
    assert_eq!(base, "https://blog.example.com");
    // The token is for "blog": it cannot be used to enter "shop" itself.
    assert_eq!(redeem(&app, token).await.status, StatusCode::UNAUTHORIZED);
    let mut validation = jsonwebtoken::Validation::default();
    validation.set_audience(&["blog"]);
    let claims = jsonwebtoken::decode::<Value>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(SSO_SECRET.as_bytes()),
        &validation,
    )
    .unwrap()
    .claims;
    assert_eq!(claims["sub"], ADMIN_EMAIL);
    let ttl = claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap();
    assert!(ttl <= 60, "short-lived token");
}

#[tokio::test]
async fn host_project_list_and_status() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let list = get(&app, "/admin/api/projects", &cookie).await.body;
    assert_eq!(list["current"], "shop");
    assert_eq!(list["sso"], true);
    let names: Vec<&str> = list["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["shop", "blog"]);
    assert_eq!(list["projects"][0]["current"], true);
    assert!(
        !list.to_string().contains("secret"),
        "the list never carries secrets"
    );

    // Outside a Docker host the apps do not answer: the status comes back as down.
    let status = get(&app, "/admin/api/projects/status", &cookie).await.body;
    assert_eq!(status["projects"].as_array().unwrap().len(), 2);
    assert_eq!(status["projects"][1]["healthy"], false);

    let reply = app
        .raw(Method::GET, "/admin/api/projects", &[], String::new())
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}

/// Percent-encoding to put JSON in the query string.
fn enc(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn names(data: &Value) -> Vec<String> {
    data["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn grid_filters_validated_by_the_catalog() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    let table = |filters: Value| {
        format!(
            "/admin/api/tables/products?sort=id&filters={}",
            enc(&filters.to_string())
        )
    };

    let expensive = get(
        &app,
        &table(json!([{ "column": "price", "op": "gte", "value": "10" }])),
        &cookie,
    )
    .await;
    assert_eq!(expensive.status, StatusCode::OK, "{}", expensive.text);
    assert_eq!(names(&expensive.body), ["Notebook", "Backpack"]);
    // The total follows the filter (and is exact).
    assert_eq!(expensive.body["total"], 2);
    assert_eq!(expensive.body["total_exact"], true);

    let contains = get(
        &app,
        &table(json!([{ "column": "name", "op": "ilike", "value": "*N*" }])),
        &cookie,
    )
    .await
    .body;
    assert_eq!(names(&contains), ["Pen", "Notebook"]);

    let negated = get(
        &app,
        &table(json!([
            { "column": "stock", "op": "eq", "value": "0", "not": true },
            { "column": "price", "op": "lt", "value": "10" },
        ])),
        &cookie,
    )
    .await
    .body;
    assert_eq!(names(&negated), ["Pen", "Ruler, 30cm"]);

    // A value of the wrong type: 400 with Postgres' message, not a 500.
    let wrong_type = get(
        &app,
        &table(json!([{ "column": "price", "op": "eq", "value": "abc" }])),
        &cookie,
    )
    .await;
    assert_eq!(
        wrong_type.status,
        StatusCode::BAD_REQUEST,
        "{}",
        wrong_type.text
    );
    assert!(wrong_type.text.contains("numeric"), "{}", wrong_type.text);

    // A column outside the catalog and an operator outside the list: refused before the database.
    for filters in [
        json!([{ "column": "missing", "op": "eq", "value": "1" }]),
        json!([{ "column": "id", "op": "in", "value": "(1,2)" }]),
    ] {
        let reply = get(&app, &table(filters), &cookie).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{}", reply.text);
    }
    let garbage = get(&app, "/admin/api/tables/products?filters=not-json", &cookie).await;
    assert_eq!(garbage.status, StatusCode::BAD_REQUEST);
    assert_eq!(garbage.body["code"], "invalid_filters");
}

#[tokio::test]
async fn export_a_table_as_csv_and_json() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let csv = get(
        &app,
        "/admin/api/tables/products/export?format=csv&sort=id",
        &cookie,
    )
    .await;
    assert_eq!(csv.status, StatusCode::OK, "{}", csv.text);
    assert_eq!(csv.headers[header::CONTENT_TYPE], "text/csv; charset=utf-8");
    assert_eq!(
        csv.headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"products.csv\""
    );
    let lines: Vec<&str> = csv.text.split("\r\n").collect();
    assert_eq!(lines[0], "\u{feff}id,name,price,stock,created_at,slug");
    assert!(lines[1].starts_with("1,Pen,2.50,100,"), "{}", lines[1]);
    // A comma inside the value: quoted field.
    assert!(
        lines[4].starts_with("4,\"Ruler, 30cm\",4.00,"),
        "{}",
        lines[4]
    );
    assert_eq!(lines.len(), 6, "header + 4 rows + empty end");

    // Export follows the grid's filters.
    let filters = enc(&json!([{ "column": "stock", "op": "eq", "value": "0" }]).to_string());
    let filtered = get(
        &app,
        &format!("/admin/api/tables/products/export?format=json&filters={filters}"),
        &cookie,
    )
    .await;
    assert_eq!(filtered.headers[header::CONTENT_TYPE], "application/json");
    let rows: Value = serde_json::from_str(&filtered.text).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["name"], "Backpack");
    // numeric goes out with Postgres' text.
    assert!(filtered.text.contains("120.00"), "{}", filtered.text);

    let error = get(
        &app,
        &format!(
            "/admin/api/tables/products/export?format=csv&filters={}",
            enc(&json!([{ "column": "id", "op": "eq", "value": "x" }]).to_string())
        ),
        &cookie,
    )
    .await;
    assert_eq!(error.status, StatusCode::BAD_REQUEST, "{}", error.text);
    let format = get(
        &app,
        "/admin/api/tables/products/export?format=xls",
        &cookie,
    )
    .await;
    assert_eq!(format.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn columns_point_to_their_foreign_key() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    app.admin_client
        .batch_execute(
            "CREATE TABLE public.reviews (
                 id int PRIMARY KEY,
                 product_id int REFERENCES public.products (id),
                 rating int
             );
             GRANT SELECT ON public.reviews TO service_role;",
        )
        .await
        .unwrap();

    // The catalog reloads by itself after the DDL.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let data = loop {
        let reply = get(&app, "/admin/api/tables/reviews", &cookie).await;
        if reply.status == StatusCode::OK {
            break reply.body;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the new table did not show up"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    let column = |name: &str| {
        data["table"]["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(
        column("product_id")["references"],
        json!({ "table": "products", "column": "id" })
    );
    assert_eq!(column("rating")["references"], Value::Null);
}

#[tokio::test]
async fn table_structure_read_from_the_postgres_catalog() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let reply = get(&app, "/admin/api/tables/products/structure", &cookie).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let data = reply.body;
    assert_eq!(data["primary_key"], json!(["id"]));
    assert_eq!(data["rls_enabled"], false);
    assert_eq!(data["comment"], "Product catalog");
    let column = |name: &str| {
        data["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(column("id")["identity"], "by default");
    assert_eq!(column("id")["primary_key"], true);
    assert_eq!(column("name")["data_type"], "character varying(80)");
    assert_eq!(column("name")["nullable"], false);
    assert_eq!(column("stock")["default"], "0");
    assert_eq!(column("created_at")["default"], "now()");
    assert_eq!(column("slug")["generated"], true);
    assert_eq!(
        data["grants"],
        json!([
            { "role": "anon", "privileges": ["select"] },
            { "role": "authenticated", "privileges": ["select"] },
            { "role": "service_role", "privileges": ["select", "insert", "update", "delete"] },
        ])
    );

    app.admin_client
        .batch_execute(
            "CREATE TABLE public.items (
                 id int PRIMARY KEY,
                 product_id int REFERENCES public.products (id) ON DELETE CASCADE,
                 code text CONSTRAINT items_code_key UNIQUE
             );",
        )
        .await
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let items = loop {
        let reply = get(&app, "/admin/api/tables/items/structure", &cookie).await;
        if reply.status == StatusCode::OK {
            break reply.body;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the new table did not show up"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    let columns = items["columns"].as_array().unwrap();
    assert_eq!(
        columns[1]["references"],
        json!({ "table": "products", "column": "id", "on_delete": "cascade", "constraint": "items_product_id_fkey" })
    );
    assert_eq!(columns[2]["unique"], "items_code_key");

    let missing = get(&app, "/admin/api/tables/missing/structure", &cookie).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

fn columns_of(structure: &Value) -> Vec<String> {
    structure["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn create_alter_and_drop_tables_from_the_panel() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    let notes = json!({
        "name": "notes",
        "comment": "The team's notes",
        "columns": [
            { "name": "id", "data_type": "bigint", "primary_key": true, "identity": true },
            { "name": "body", "data_type": "text", "nullable": false },
            { "name": "product_id", "data_type": "integer",
              "references": { "table": "products", "column": "id", "on_delete": "cascade" } },
            { "name": "created_at", "data_type": "timestamptz", "nullable": false, "default": "now()" },
        ],
        "grants": [{ "role": "anon", "privileges": ["select"] }],
    });

    // Preview: returns the SQL and creates nothing.
    let preview = send(
        &app,
        Method::POST,
        "/admin/api/tables",
        &cookie,
        json!({ "table": notes, "preview": true }),
    )
    .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.text);
    assert!(
        preview.body["sql"][0]
            .as_str()
            .unwrap()
            .starts_with("CREATE TABLE \"public\".\"notes\"")
    );
    assert_eq!(
        get(&app, "/admin/api/tables/notes", &cookie).await.status,
        StatusCode::NOT_FOUND
    );

    let created = send(
        &app,
        Method::POST,
        "/admin/api/tables",
        &cookie,
        json!({ "table": notes }),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text);
    // The catalog already reloaded: the table is in the panel and the REST API, no waiting.
    let structure = get(&app, "/admin/api/tables/notes/structure", &cookie)
        .await
        .body;
    assert_eq!(structure["rls_enabled"], true);
    assert_eq!(
        structure["grants"][0],
        json!({ "role": "anon", "privileges": ["select"] })
    );
    let rest = app
        .raw(Method::GET, "/rest/v1/notes", &[], String::new())
        .await;
    assert_eq!(rest.status, StatusCode::OK, "{}", rest.text);
    assert_eq!(
        rest.body,
        json!([]),
        "RLS on and no policies: anon sees nothing"
    );

    // Several changes in one transaction.
    let altered = send(&app, Method::PATCH, "/admin/api/tables/notes", &cookie, json!({ "actions": [
        { "action": "add_column", "column": { "name": "votes", "data_type": "integer", "default": "0", "nullable": false } },
        { "action": "rename_column", "from": "body", "to": "content" },
        { "action": "set_type", "column": "content", "data_type": "varchar(500)" },
        { "action": "set_unique", "column": "content", "unique": true },
        { "action": "set_grants", "grant": { "role": "authenticated", "privileges": ["select", "insert"] } },
    ] })).await;
    assert_eq!(altered.status, StatusCode::OK, "{}", altered.text);
    let structure = get(&app, "/admin/api/tables/notes/structure", &cookie)
        .await
        .body;
    assert_eq!(
        columns_of(&structure),
        ["id", "content", "product_id", "created_at", "votes"]
    );
    let content = &structure["columns"][1];
    assert_eq!(content["data_type"], "character varying(500)");
    assert!(content["unique"].is_string());
    assert_eq!(
        structure["grants"][1]["privileges"],
        json!(["select", "insert"])
    );

    // Atomicity: the second action fails (impossible cast) and the first one is not kept.
    let failed = send(
        &app,
        Method::PATCH,
        "/admin/api/tables/notes",
        &cookie,
        json!({ "actions": [
        { "action": "add_column", "column": { "name": "draft", "data_type": "boolean" } },
        { "action": "set_type", "column": "created_at", "data_type": "integer" },
    ] }),
    )
    .await;
    assert_eq!(failed.status, StatusCode::BAD_REQUEST, "{}", failed.text);
    let structure = get(&app, "/admin/api/tables/notes/structure", &cookie)
        .await
        .body;
    assert!(!columns_of(&structure).contains(&"draft".to_owned()));

    // An expression with a second statement: the extended protocol refuses it.
    let injection = send(
        &app,
        Method::PATCH,
        "/admin/api/tables/notes",
        &cookie,
        json!({ "actions": [
        { "action": "add_column", "column": { "name": "x", "data_type": "text",
          "default": "'a'); DROP TABLE public.products; --" } },
    ] }),
    )
    .await;
    assert_eq!(
        injection.status,
        StatusCode::BAD_REQUEST,
        "{}",
        injection.text
    );
    assert_eq!(
        get(&app, "/admin/api/tables/products", &cookie)
            .await
            .status,
        StatusCode::OK
    );

    // Validation before the database.
    let bad_type = send(
        &app,
        Method::POST,
        "/admin/api/tables",
        &cookie,
        json!({ "table": {
        "name": "t", "columns": [{ "name": "a", "data_type": "money" }] } }),
    )
    .await;
    assert_eq!(bad_type.status, StatusCode::BAD_REQUEST);
    assert!(bad_type.text.contains("unknown type"), "{}", bad_type.text);
    assert_eq!(bad_type.body["code"], "unknown_type");
    assert_eq!(bad_type.body["params"], json!({ "type": "money" }));

    // products is referenced by notes: without CASCADE Postgres refuses.
    let blocked = app
        .raw(
            Method::DELETE,
            "/admin/api/tables/products",
            &[("cookie", cookie.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(blocked.status, StatusCode::BAD_REQUEST, "{}", blocked.text);
    let dropped = app
        .raw(
            Method::DELETE,
            "/admin/api/tables/notes",
            &[("cookie", cookie.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(dropped.status, StatusCode::OK, "{}", dropped.text);
    assert_eq!(
        get(&app, "/admin/api/tables/notes", &cookie).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.raw(Method::GET, "/rest/v1/notes", &[], String::new())
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let types = get(&app, "/admin/api/types", &cookie).await.body;
    assert!(
        types["base"]
            .as_array()
            .unwrap()
            .contains(&json!("timestamptz"))
    );
    assert_eq!(types["enums"], json!(["priority"]));
}

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
async fn service_role_token_issued_by_the_panel() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let anon = app
        .raw(Method::GET, "/rest/v1/secrets", &[], String::new())
        .await;
    assert_ne!(
        anon.status,
        StatusCode::OK,
        "secrets only has a GRANT for service_role"
    );

    let reply = send(
        &app,
        Method::POST,
        "/admin/api/tokens/service-role",
        &cookie,
        json!({ "days": 7 }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");
    let token = reply.body["token"].as_str().unwrap().to_owned();
    let bearer = format!("Bearer {token}");
    let service = app
        .raw(
            Method::GET,
            "/rest/v1/secrets",
            &[("authorization", bearer.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(service.status, StatusCode::OK, "{}", service.text);

    for days in [0, 3651] {
        let bad = send(
            &app,
            Method::POST,
            "/admin/api/tokens/service-role",
            &cookie,
            json!({ "days": days }),
        )
        .await;
        assert_eq!(bad.status, StatusCode::BAD_REQUEST);
        assert_eq!(bad.body["code"], "invalid_token_validity");
        assert_eq!(bad.body["params"], json!({ "max": 3650 }));
    }
    // Without a panel session, no token.
    let outsider = app
        .raw(
            Method::POST,
            "/admin/api/tokens/service-role",
            &[JSON],
            json!({ "days": 7 }).to_string(),
        )
        .await;
    assert_eq!(outsider.status, StatusCode::UNAUTHORIZED);
}

async fn password_login(app: &TestApp, email: &str, password: &str) -> Reply {
    app.raw(
        Method::POST,
        "/auth/v1/token?grant_type=password",
        &[JSON],
        json!({ "email": email, "password": password }).to_string(),
    )
    .await
}

#[tokio::test]
async fn create_user_and_reset_password_from_the_panel() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let created = send(
        &app,
        Method::POST,
        "/admin/api/users",
        &cookie,
        json!({ "email": "  New@Example.com ", "password": "initial-password-123" }),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text);
    // Same rules as public signup: normalized email.
    assert_eq!(created.body["email"], "new@example.com");
    let id = created.body["id"].as_str().unwrap().to_owned();

    // The created user signs in through the public API, with the same hash as signup.
    let session = password_login(&app, "new@example.com", "initial-password-123").await;
    assert_eq!(session.status, StatusCode::OK, "{}", session.text);
    let refresh = session.body["refresh_token"].as_str().unwrap().to_owned();

    for (body, status, code) in [
        (
            json!({ "email": "new@example.com", "password": "other-password-123" }),
            StatusCode::CONFLICT,
            "user_already_exists",
        ),
        (
            json!({ "email": "no-at-sign", "password": "valid-password-123" }),
            StatusCode::BAD_REQUEST,
            "invalid_email",
        ),
        (
            json!({ "email": "short@example.com", "password": "1234567" }),
            StatusCode::BAD_REQUEST,
            "password_too_short",
        ),
    ] {
        let reply = send(&app, Method::POST, "/admin/api/users", &cookie, body).await;
        assert_eq!(reply.status, status, "{}", reply.text);
        assert_eq!(reply.body["code"], code, "{}", reply.text);
    }

    // Reset: the old password stops working and the open session is ended.
    let reset = send(
        &app,
        Method::PUT,
        &format!("/admin/api/users/{id}/password"),
        &cookie,
        json!({ "password": "new-password-456" }),
    )
    .await;
    assert_eq!(reset.status, StatusCode::OK, "{}", reset.text);
    assert_eq!(reset.body["sessions_revoked"], 1);
    assert_eq!(
        password_login(&app, "new@example.com", "initial-password-123")
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        password_login(&app, "new@example.com", "new-password-456")
            .await
            .status,
        StatusCode::OK
    );
    let reused = app
        .raw(
            Method::POST,
            "/auth/v1/token?grant_type=refresh_token",
            &[JSON],
            json!({ "refresh_token": refresh }).to_string(),
        )
        .await;
    assert_ne!(
        reused.status,
        StatusCode::OK,
        "a session from before the password change cannot refresh"
    );

    let weak = send(
        &app,
        Method::PUT,
        &format!("/admin/api/users/{id}/password"),
        &cookie,
        json!({ "password": "short" }),
    )
    .await;
    assert_eq!(weak.status, StatusCode::BAD_REQUEST);
    let missing = send(
        &app,
        Method::PUT,
        "/admin/api/users/00000000-0000-0000-0000-000000000000/password",
        &cookie,
        json!({ "password": "valid-password-123" }),
    )
    .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.body["code"], "user_not_found");
    let bad_id = send(
        &app,
        Method::PUT,
        "/admin/api/users/not-a-uuid/password",
        &cookie,
        json!({ "password": "valid-password-123" }),
    )
    .await;
    assert_eq!(bad_id.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn admin_profile_photo() {
    // Minimal PNG (signature + start of IHDR) and an SVG, in base64.
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUg==";
    const SVG: &str = "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=";
    let app = TestApp::spawn().await;

    for path in ["/admin/api/profile", "/admin/api/profile/avatar"] {
        let reply = app.raw(Method::GET, path, &[], String::new()).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{path}");
    }

    let cookie = login(&app).await;
    let profile = get(&app, "/admin/api/profile", &cookie).await;
    assert_eq!(profile.status, StatusCode::OK, "{}", profile.text);
    assert_eq!(profile.body["email"], ADMIN_EMAIL);
    assert!(profile.body["avatar"].is_null());
    let reply = get(&app, "/admin/api/profile/avatar", &cookie).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);

    // Only PNG, JPEG or WebP recognised by their bytes; size is limited.
    let too_big = "A".repeat(349_528); // 262,146 decoded bytes
    for image in [SVG, "not base64!", "", too_big.as_str()] {
        let reply = send(
            &app,
            Method::PUT,
            "/admin/api/profile/avatar",
            &cookie,
            json!({ "image": image }),
        )
        .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{}", reply.text);
    }

    // CSRF: another origin does not change the photo.
    let reply = app
        .raw(
            Method::PUT,
            "/admin/api/profile/avatar",
            &[
                ("cookie", &cookie),
                JSON,
                ("origin", "https://malicious.example"),
            ],
            json!({ "image": PNG }).to_string(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);

    let reply = send(
        &app,
        Method::PUT,
        "/admin/api/profile/avatar",
        &cookie,
        json!({ "image": PNG }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let version = reply.body["avatar"].as_i64().expect("photo version");
    let profile = get(&app, "/admin/api/profile", &cookie).await;
    assert_eq!(profile.body["avatar"], version);

    let reply = get(&app, "/admin/api/profile/avatar", &cookie).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(reply.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(reply.text.ends_with("IHDR"));

    // The table is out of reach of the API roles.
    let out = sql(
        &app,
        &cookie,
        "select has_schema_privilege('anon', 'nelcota', 'usage'),
                has_schema_privilege('authenticated', 'nelcota', 'usage'),
                has_schema_privilege('service_role', 'nelcota', 'usage')",
    )
    .await;
    assert_eq!(out["results"][0]["rows"], json!([["f", "f", "f"]]));

    let reply = send(
        &app,
        Method::DELETE,
        "/admin/api/profile/avatar",
        &cookie,
        json!({}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.body["avatar"].is_null());
    let reply = get(&app, "/admin/api/profile/avatar", &cookie).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

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
