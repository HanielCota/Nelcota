//! Tests of the automatic REST API against a real Postgres: CRUD, filters, RLS
//! on every verb, SQL injection, RPC, OpenAPI, catalog reload and a basic
//! performance check.

mod common;

use std::time::{Duration, Instant};

use axum::http::{Method, StatusCode, header};
use common::*;
use serde_json::{Value, json};

const REPR: (&str, &str) = ("prefer", "return=representation");

fn ids(body: &Value) -> Vec<i64> {
    body.as_array()
        .unwrap_or_else(|| panic!("expected an array: {body}"))
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect()
}

fn names(body: &Value) -> Vec<&str> {
    body.as_array()
        .unwrap_or_else(|| panic!("expected an array: {body}"))
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn full_crud_with_representation() {
    let app = TestApp::spawn().await;
    let a = user_token(app.user_a);

    // POST with representation: user_id comes from the DEFAULT auth.uid().
    let created = app
        .request_with(
            Method::POST,
            "/rest/v1/todos",
            Some(&a),
            Some(json!({ "title": "new", "tags": ["home", "urgent"], "extra": { "x": 1 } })),
            &[REPR],
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let row = &created.body[0];
    assert_eq!(row["user_id"], app.user_a.to_string());
    assert_eq!(row["priority"], "medium");
    assert_eq!(row["tags"], json!(["home", "urgent"]));
    assert_eq!(row["extra"], json!({ "x": 1 }));
    let id = row["id"].as_i64().unwrap();

    // Minimal POST: 201 without a body.
    let minimal = app
        .post(
            "/rest/v1/todos",
            Some(&a),
            json!([{ "title": "x" }, { "title": "y" }]),
        )
        .await;
    assert_eq!(minimal.status, StatusCode::CREATED);
    assert_eq!(minimal.body, Value::Null);

    let (status, body) = app
        .get(
            &format!("/rest/v1/todos?id=eq.{id}&select=id,title,done"),
            Some(&a),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([{ "id": id, "title": "new", "done": false }]));

    let patched = app
        .request_with(
            Method::PATCH,
            &format!("/rest/v1/todos?id=eq.{id}&select=id,done"),
            Some(&a),
            Some(json!({ "done": true, "priority": "high" })),
            &[REPR],
        )
        .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    assert_eq!(patched.body, json!([{ "id": id, "done": true }]));

    let minimal = app
        .request(
            Method::PATCH,
            &format!("/rest/v1/todos?id=eq.{id}"),
            Some(&a),
            Some(json!({ "title": "renamed" })),
        )
        .await;
    assert_eq!(minimal.status, StatusCode::NO_CONTENT);

    let deleted = app
        .request_with(
            Method::DELETE,
            &format!("/rest/v1/todos?id=eq.{id}"),
            Some(&a),
            None,
            &[REPR],
        )
        .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert_eq!(deleted.body[0]["title"], "renamed");

    let (_, body) = app
        .get(&format!("/rest/v1/todos?id=eq.{id}"), Some(&a))
        .await;
    assert_eq!(body, json!([]));
}

#[tokio::test]
async fn rls_holds_on_every_verb() {
    let app = TestApp::spawn().await;
    let a = user_token(app.user_a);
    let b_id: i64 = app
        .admin_client
        .query_one(
            "SELECT id FROM public.todos WHERE user_id = $1",
            &[&app.user_b],
        )
        .await
        .unwrap()
        .get(0);

    // GET: A does not see B's row, not even filtering by its id.
    let (_, body) = app
        .get(&format!("/rest/v1/todos?id=eq.{b_id}"), Some(&a))
        .await;
    assert_eq!(body, json!([]));

    // PATCH/DELETE on B's row: zero rows affected.
    for method in [Method::PATCH, Method::DELETE] {
        let reply = app
            .request_with(
                method.clone(),
                &format!("/rest/v1/todos?id=eq.{b_id}"),
                Some(&a),
                (method == Method::PATCH).then(|| json!({ "title": "hijacked" })),
                &[REPR],
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{method}");
        assert_eq!(reply.body, json!([]), "{method}");
    }

    // POST on behalf of B: policy violation (WITH CHECK).
    let reply = app
        .post(
            "/rest/v1/todos",
            Some(&a),
            json!({ "title": "forged", "user_id": app.user_b }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN, "{}", reply.body);

    // PATCH trying to hand one's own row to B: also blocked.
    let reply = app
        .request(
            Method::PATCH,
            "/rest/v1/todos?title=eq.task%20of%20A",
            Some(&a),
            Some(json!({ "user_id": app.user_b })),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);

    // B's row stays intact.
    let title: String = app
        .admin_client
        .query_one("SELECT title FROM public.todos WHERE id = $1", &[&b_id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(title, "task of B");

    // anon: no GRANT, 401 on every verb.
    for (method, body) in [
        (Method::GET, None),
        (Method::POST, Some(json!({ "title": "x" }))),
        (Method::PATCH, Some(json!({ "title": "x" }))),
        (Method::DELETE, None),
    ] {
        let path = if method == Method::POST {
            "/rest/v1/todos"
        } else {
            "/rest/v1/todos?id=gt.0"
        };
        let reply = app.request(method.clone(), path, None, body).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{method}");
    }

    // authenticated without a write GRANT on products: 403. service_role: ok.
    let reply = app
        .post(
            "/rest/v1/products",
            Some(&a),
            json!({ "name": "X", "price": 1 }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    let reply = app
        .post(
            "/rest/v1/products",
            Some(&service_token()),
            json!({ "name": "X", "price": 1 }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED);

    // Table without a GRANT for users.
    let (status, _) = app.get("/rest/v1/secrets", Some(&a)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = app.get("/rest/v1/secrets", Some(&service_token())).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn filters_order_paging_and_count() {
    let app = TestApp::spawn().await;
    let get = |q: &str| {
        let path = format!("/rest/v1/products?{q}");
        let app = &app;
        async move { app.get(&path, None).await }
    };

    let (_, b) = get("price=gt.10&order=price.desc").await;
    assert_eq!(names(&b), ["Backpack", "Notebook"]);
    let (_, b) = get("stock=eq.0").await;
    assert_eq!(names(&b), ["Backpack"]);
    let (_, b) = get("stock=neq.0&order=name").await;
    assert_eq!(names(&b), ["Notebook", "Pen", "Ruler, 30cm"]);
    let (_, b) = get("price=lte.4&price=gte.3").await;
    assert_eq!(names(&b), ["Ruler, 30cm"]);
    let (_, b) = get("name=in.(Pen,\"Ruler, 30cm\")&order=id").await;
    assert_eq!(names(&b), ["Pen", "Ruler, 30cm"]);
    let (_, b) = get("name=in.()").await;
    assert_eq!(b, json!([]));
    let (_, b) = get("name=like.*e*&order=name").await;
    assert_eq!(names(&b), ["Notebook", "Pen", "Ruler, 30cm"]);
    let (_, b) = get("name=ilike.*PACK").await;
    assert_eq!(names(&b), ["Backpack"]);
    let (_, b) = get("name=not.like.P*&order=name.desc").await;
    assert_eq!(names(&b), ["Ruler, 30cm", "Notebook", "Backpack"]);
    let (_, b) = get("slug=eq.backpack&select=name,slug").await;
    assert_eq!(b, json!([{ "name": "Backpack", "slug": "backpack" }]));
    let (_, b) = get("created_at=not.is.null&select=id&order=id&limit=2&offset=1").await;
    assert_eq!(ids(&b), [2, 3]);

    // Content-Range and count=exact.
    let reply = app
        .request_with(
            Method::GET,
            "/rest/v1/products?order=id&limit=2&offset=1",
            None,
            None,
            &[("prefer", "count=exact")],
        )
        .await;
    assert_eq!(reply.headers[header::CONTENT_RANGE], "1-2/4");
    let reply = app
        .request(Method::GET, "/rest/v1/products?stock=gt.1000", None, None)
        .await;
    assert_eq!(reply.headers[header::CONTENT_RANGE], "*/*");

    // Filters on enum, array and boolean (as a user).
    let a = user_token(app.user_a);
    // Batch with different keys: missing columns get the DEFAULT.
    let reply = app
        .post(
            "/rest/v1/todos",
            Some(&a),
            json!([
                { "title": "high", "priority": "high", "tags": ["x"] },
                { "title": "low", "priority": "low", "done": true },
            ]),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    let (_, b) = app
        .get("/rest/v1/todos?priority=eq.high&select=title", Some(&a))
        .await;
    assert_eq!(b, json!([{ "title": "high" }]));
    let (_, b) = app
        .get("/rest/v1/todos?done=is.true&select=title", Some(&a))
        .await;
    assert_eq!(b, json!([{ "title": "low" }]));
    let (_, b) = app
        .get(
            "/rest/v1/todos?priority=in.(high,low)&select=title&order=title",
            Some(&a),
        )
        .await;
    assert_eq!(b, json!([{ "title": "high" }, { "title": "low" }]));
}

#[tokio::test]
async fn max_rows_caps_reads() {
    let app = TestApp::spawn_with(Options {
        max_rows: Some(2),
        ..Options::default()
    })
    .await;
    let (_, b) = app.get("/rest/v1/products?order=id", None).await;
    assert_eq!(ids(&b), [1, 2]);
    let (_, b) = app.get("/rest/v1/products?order=id&limit=100", None).await;
    assert_eq!(ids(&b).len(), 2);
}

#[tokio::test]
async fn data_errors_become_400() {
    let app = TestApp::spawn().await;
    let s = service_token();
    for (case, body) in [
        ("check", json!({ "name": "X", "price": -1 })),
        ("not null", json!({ "price": 1 })),
        ("type", json!({ "name": "X", "price": "expensive" })),
        (
            "generated column",
            json!({ "name": "X", "price": 1, "slug": "y" }),
        ),
        (
            "unknown column",
            json!({ "name": "X", "price": 1, "color": "blue" }),
        ),
    ] {
        let reply = app.post("/rest/v1/products", Some(&s), body).await;
        assert_eq!(
            reply.status,
            StatusCode::BAD_REQUEST,
            "{case}: {}",
            reply.body
        );
    }
    let (status, _) = app.get("/rest/v1/products?price=eq.abc", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = app.get("/rest/v1/products?id=gt.1&stock=lt.x", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Duplicate PK: 409.
    let reply = app
        .post(
            "/rest/v1/products",
            Some(&s),
            json!({ "id": 1, "name": "X", "price": 1 }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);

    // A body that is not JSON.
    let reply = app
        .request_with(Method::POST, "/rest/v1/products", Some(&s), None, &[])
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);

    // PATCH/DELETE without a filter are refused.
    let reply = app
        .request(Method::DELETE, "/rest/v1/products", Some(&s), None)
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    let reply = app
        .request(
            Method::PATCH,
            "/rest/v1/products",
            Some(&s),
            Some(json!({ "stock": 0 })),
        )
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);

    let (status, _) = app.get("/rest/v1/does_not_exist", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sql_injection_in_identifiers_and_values() {
    let app = TestApp::spawn().await;
    let s = service_token();

    // Malicious identifiers: rejected before reaching the database.
    for path in [
        "/rest/v1/products;DROP%20TABLE%20products",
        "/rest/v1/products%22;DROP%20TABLE%20products;--",
        "/rest/v1/pg_catalog.pg_authid",
        "/rest/v1/..%2Fauth.users",
    ] {
        let (status, _) = app.get(path, Some(&s)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    for query in [
        "select=name,(SELECT%20password%20FROM%20auth.users)",
        "select=name%22,%22price",
        "select=*;DROP%20TABLE%20products",
        "order=name;DROP%20TABLE%20products",
        "order=(SELECT%201)",
        "name%22%3D%27x%27%20OR%201%3D1--=eq.1",
        "id=eq.1&1=1",
        "limit=1;DROP%20TABLE%20products",
        "id=in.(1)%29%20OR%20(1=1",
    ] {
        let (status, body) = app
            .get(&format!("/rest/v1/products?{query}"), Some(&s))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {body}");
    }

    // Malicious values: treated as data.
    let payload = "x'); DROP TABLE products; --";
    let (status, body) = app
        .get(
            &format!("/rest/v1/products?name=eq.{}", urlencode(payload)),
            Some(&s),
        )
        .await;
    assert_eq!((status, body), (StatusCode::OK, json!([])));
    let (status, _) = app
        .get(
            &format!(
                "/rest/v1/products?name=in.({},\"a\\\"b\")",
                urlencode(payload)
            ),
            Some(&s),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let created = app
        .request_with(
            Method::POST,
            "/rest/v1/products",
            Some(&s),
            Some(json!({ "name": payload, "price": 1 })),
            &[REPR],
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    assert_eq!(created.body[0]["name"], payload);

    // Malicious keys in the body and function names.
    let reply = app
        .post(
            "/rest/v1/products",
            Some(&s),
            json!({ "name\" text); DROP TABLE products; --": "x" }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    let reply = app
        .post(
            "/rest/v1/rpc/add;DROP%20TABLE%20products",
            Some(&s),
            json!({}),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    let reply = app
        .post(
            "/rest/v1/rpc/add",
            Some(&s),
            json!({ "a) ; DROP TABLE products; --": 1 }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);

    // Everything is still there.
    let count: i64 = app
        .admin_client
        .query_one("SELECT count(*) FROM public.products", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 5);
}

fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[tokio::test]
async fn rpc_calls_functions_under_the_jwt_role() {
    let app = TestApp::spawn().await;
    let a = user_token(app.user_a);

    let reply = app.post("/rest/v1/rpc/add", None, json!({ "a": 1 })).await;
    assert_eq!((reply.status, reply.body), (StatusCode::OK, json!(11)));
    let reply = app
        .post("/rest/v1/rpc/add", None, json!({ "a": 1, "b": 2 }))
        .await;
    assert_eq!(reply.body, json!(3));
    let reply = app
        .post("/rest/v1/rpc/add", None, json!({ "a": 1, "c": 2 }))
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    let reply = app.post("/rest/v1/rpc/add", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);

    // SETOF functions obey the table's RLS.
    let reply = app
        .post("/rest/v1/rpc/my_open_todos", Some(&a), json!({}))
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body.as_array().unwrap().len(), 1);
    assert_eq!(reply.body[0]["title"], "task of A");

    // auth.uid()/auth.role() inside the function reflect the JWT.
    let reply = app.post("/rest/v1/rpc/who_am_i", Some(&a), json!({})).await;
    assert_eq!(
        reply.body,
        json!({ "uid": app.user_a, "role": "authenticated" })
    );
    let reply = app
        .request(Method::POST, "/rest/v1/rpc/who_am_i", None, None)
        .await;
    assert_eq!(reply.body, json!({ "uid": null, "role": "anon" }));

    let reply = app.post("/rest/v1/rpc/nothing", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let reply = app.post("/rest/v1/rpc/fail", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert!(
        reply.body["message"]
            .as_str()
            .unwrap()
            .contains("business rule")
    );
    let reply = app
        .post("/rest/v1/rpc/does_not_exist", None, json!({}))
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);

    // EXECUTE revoked from PUBLIC: anon 401, authenticated 403, service ok.
    let reply = app.post("/rest/v1/rpc/service_only", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    let reply = app
        .post("/rest/v1/rpc/service_only", Some(&a), json!({}))
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    let reply = app
        .post(
            "/rest/v1/rpc/service_only",
            Some(&service_token()),
            json!({}),
        )
        .await;
    assert_eq!(reply.body, json!("ok"));
}

#[tokio::test]
async fn openapi_follows_the_role_privileges() {
    let app = TestApp::spawn().await;

    let (status, anon) = app.get("/rest/v1/", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(anon["openapi"], "3.0.3");
    let paths = anon["paths"].as_object().unwrap();
    assert!(paths.contains_key("/products"));
    assert!(paths["/products"].get("get").is_some());
    assert!(paths["/products"].get("post").is_none());
    assert!(!paths.contains_key("/todos"));
    assert!(!paths.contains_key("/secrets"));
    assert!(paths.contains_key("/rpc/add"));
    assert!(!paths.contains_key("/rpc/service_only"));
    let product = &anon["components"]["schemas"]["products"];
    assert_eq!(product["properties"]["price"]["type"], "number");
    assert_eq!(product["properties"]["id"]["type"], "integer");
    assert_eq!(product["description"], "Product catalog");

    let (_, user) = app.get("/rest/v1/", Some(&user_token(app.user_a))).await;
    let todos = &user["paths"]["/todos"];
    for verb in ["get", "post", "patch", "delete"] {
        assert!(todos.get(verb).is_some(), "{verb}");
    }
    let priority = &user["components"]["schemas"]["todos"]["properties"]["priority"];
    assert_eq!(priority["enum"], json!(["low", "medium", "high"]));

    let (_, service) = app.get("/rest/v1/", Some(&service_token())).await;
    assert!(service["paths"].get("/secrets").is_some());
    assert!(service["paths"].get("/rpc/service_only").is_some());
}

#[tokio::test]
async fn catalog_reloads_on_its_own_after_ddl() {
    let app = TestApp::spawn().await;
    let (status, _) = app.get("/rest/v1/news", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    app.admin_client
        .batch_execute(
            "CREATE TABLE public.news (id int PRIMARY KEY, body text);
             INSERT INTO public.news VALUES (1, 'hello');
             GRANT SELECT ON public.news TO anon;",
        )
        .await
        .unwrap();

    // The event trigger notifies; the listener reloads within ~100 ms.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (status, body) = app.get("/rest/v1/news", None).await;
        if status == StatusCode::OK {
            assert_eq!(body, json!([{ "id": 1, "body": "hello" }]));
            break;
        }
        assert!(Instant::now() < deadline, "catalog did not reload");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // New columns show up too.
    app.admin_client
        .batch_execute("ALTER TABLE public.news ADD COLUMN read boolean DEFAULT false")
        .await
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while app
        .catalog
        .get()
        .table("news")
        .unwrap()
        .column("read")
        .is_none()
    {
        assert!(Instant::now() < deadline, "the new column did not show up");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (status, _) = app.get("/rest/v1/news?read=is.false", None).await;
    assert_eq!(status, StatusCode::OK);
}

/// Not a benchmark (that lives in `bench/`): it only ensures a simple read does
/// not degrade absurdly, even in a debug build.
#[tokio::test]
async fn basic_read_performance() {
    let app = TestApp::spawn().await;
    app.admin_client
        .batch_execute(
            "INSERT INTO public.products (name, price, stock)
             SELECT 'product ' || i, (i % 500) + 0.99, i % 7 FROM generate_series(1, 20000) i;
             ANALYZE public.products;",
        )
        .await
        .unwrap();

    let requests = 300;
    let started = Instant::now();
    for i in 0..requests {
        let (status, body) = app
            .get(
                &format!("/rest/v1/products?stock=eq.{}&order=id&limit=20", i % 7),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 20);
    }
    let average = started.elapsed() / requests;
    println!("simple read: average of {average:?} per request ({requests} requests)");
    assert!(average < Duration::from_millis(100), "average {average:?}");
}

#[tokio::test]
async fn or_and_groups_filter_like_postgrest() {
    let app = TestApp::spawn().await;
    let get = |q: &str| {
        let path = format!("/rest/v1/products?{q}");
        let app = &app;
        async move { app.get(&path, None).await }
    };

    let (_, b) = get("or=(stock.eq.0,price.lt.3)&order=name").await;
    assert_eq!(names(&b), ["Backpack", "Pen"]);
    // Nested group, and a quoted value with a comma.
    let (_, b) = get("or=(name.eq.\"Ruler, 30cm\",and(price.gt.10,stock.gt.0))&order=name").await;
    assert_eq!(names(&b), ["Notebook", "Ruler, 30cm"]);
    let (_, b) = get("not.or=(stock.eq.0,price.lt.3)&order=name").await;
    assert_eq!(names(&b), ["Notebook", "Ruler, 30cm"]);
    // A tree is ANDed with the other filters.
    let (_, b) = get("or=(price.gt.100,price.lt.3)&stock=gt.0").await;
    assert_eq!(names(&b), ["Pen"]);
    let (_, b) = get("and=(price.gte.4,price.lte.15)&order=price").await;
    assert_eq!(names(&b), ["Ruler, 30cm", "Notebook"]);

    let (status, b) = get("or=(name.eq.Pen").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(b["code"], "invalid_query");
}

/// An `or` cannot widen what RLS allows: the policy is ANDed by Postgres.
#[tokio::test]
async fn rls_still_filters_inside_or() {
    let app = TestApp::spawn().await;
    let a = user_token(app.user_a);
    let b = user_token(app.user_b);
    let titles = |body: &Value| -> Vec<String> {
        body.as_array()
            .unwrap()
            .iter()
            .map(|r| r["title"].as_str().unwrap().to_owned())
            .collect()
    };

    let path = format!(
        "/rest/v1/todos?or=(user_id.eq.{},title.eq.task of B,title.eq.task of A)",
        app.user_b
    );
    let (status, body) = app.get(&path, Some(&a)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), ["task of A"]);

    // PATCH with the same tree changes only A's row.
    let reply = app
        .request_with(
            Method::PATCH,
            "/rest/v1/todos?or=(title.eq.task of A,title.eq.task of B)",
            Some(&a),
            Some(json!({ "done": true })),
            &[REPR],
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(titles(&reply.body), ["task of A"]);
    let (_, theirs) = app.get("/rest/v1/todos", Some(&b)).await;
    assert_eq!(theirs[0]["done"], false, "B's row is untouched");
}
