//! Panel projects integration scenarios.
use crate::common::panel::{get, login};
use crate::common::*;
use axum::http::{Method, StatusCode};

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
