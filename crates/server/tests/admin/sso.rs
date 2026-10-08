//! Panel sso integration scenarios.
use crate::common::panel::{JSON, get, send};
use crate::common::*;
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};

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
