//! Panel assets integration scenarios.
use crate::common::*;
use axum::http::{Method, StatusCode, header};

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
