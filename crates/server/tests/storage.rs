//! Storage: the `storage` schema and its policies, against a real Postgres 17.

mod common;

use common::*;
use nelcota_core::{Claims, db};
use serde_json::json;
use uuid::Uuid;

/// Runs `sql` as the given claims, the way a storage request does.
async fn rows_as(app: &TestApp, claims: serde_json::Value, sql: &str) -> Vec<String> {
    let mut client = app.pool.get().await.unwrap();
    let tx = db::begin_request(&mut client, &Claims::from_payload(claims).unwrap())
        .await
        .unwrap();
    let rows = tx.query(sql, &[]).await.unwrap();
    rows.iter().map(|r| r.get(0)).collect()
}

async fn seed(app: &TestApp) {
    app.admin_client
        .batch_execute(&format!(
            "INSERT INTO storage.buckets (id) VALUES ('docs');
             INSERT INTO storage.objects (bucket_id, name, owner, version, size, mime_type, etag)
             VALUES ('docs', 'a.txt', '{a}', gen_random_uuid(), 1, 'text/plain', 'x'),
                    ('docs', 'b.txt', '{b}', gen_random_uuid(), 1, 'text/plain', 'y');",
            a = app.user_a,
            b = app.user_b,
        ))
        .await
        .unwrap();
}

#[tokio::test]
async fn files_are_closed_until_a_policy_opens_them() {
    let app = TestApp::spawn().await;
    seed(&app).await;
    let names = "SELECT name FROM storage.objects ORDER BY name";
    let user_a = json!({ "role": "authenticated", "sub": app.user_a });

    assert!(
        rows_as(&app, json!({ "role": "anon" }), names)
            .await
            .is_empty()
    );
    assert!(rows_as(&app, user_a.clone(), names).await.is_empty());
    assert!(
        rows_as(&app, user_a.clone(), "SELECT id FROM storage.buckets")
            .await
            .is_empty()
    );
    assert_eq!(
        rows_as(&app, json!({ "role": "service_role" }), names).await,
        ["a.txt", "b.txt"]
    );

    app.admin_client
        .batch_execute(
            "CREATE POLICY own ON storage.objects FOR SELECT TO authenticated
                 USING (owner = auth.uid())",
        )
        .await
        .unwrap();
    assert_eq!(rows_as(&app, user_a, names).await, ["a.txt"]);
}

#[tokio::test]
async fn path_helpers() {
    let app = TestApp::spawn().await;
    let row = app
        .admin_client
        .query_one(
            "SELECT storage.foldername('a/b/c.PNG'), storage.filename('a/b/c.PNG'),
                    storage.extension('a/b/c.PNG'), storage.foldername('c.png'),
                    storage.extension('a.b/noext')",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, Vec<String>>(0), ["a", "b"]);
    assert_eq!(row.get::<_, String>(1), "c.PNG");
    assert_eq!(row.get::<_, String>(2), "png");
    assert!(row.get::<_, Vec<String>>(3).is_empty());
    assert_eq!(row.get::<_, String>(4), "");
}

#[tokio::test]
async fn bucket_names_and_owner_default() {
    let app = TestApp::spawn().await;
    for bad in ["", "Upper", "-dash", "has space", "a/b", "sign", &"x".repeat(64)] {
        assert!(
            app.admin_client
                .execute("INSERT INTO storage.buckets (id) VALUES ($1)", &[&bad])
                .await
                .is_err(),
            "{bad:?}"
        );
    }
    app.admin_client
        .batch_execute(
            "INSERT INTO storage.buckets (id) VALUES ('avatars');
             CREATE POLICY ins ON storage.objects FOR INSERT TO authenticated WITH CHECK (true);",
        )
        .await
        .unwrap();

    let user = Uuid::new_v4();
    let mut client = app.pool.get().await.unwrap();
    let claims = Claims::from_payload(json!({ "role": "authenticated", "sub": user })).unwrap();
    let tx = db::begin_request(&mut client, &claims).await.unwrap();
    tx.execute(
        "INSERT INTO storage.objects (bucket_id, name, version, size, mime_type, etag)
         VALUES ('avatars', 'me.png', gen_random_uuid(), 3, 'image/png', 'e')",
        &[],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let owner: Uuid = app
        .admin_client
        .query_one("SELECT owner FROM storage.objects", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(owner, user);

    // A bucket with files cannot be dropped.
    assert!(
        app.admin_client
            .execute("DELETE FROM storage.buckets WHERE id = 'avatars'", &[])
            .await
            .is_err()
    );
}
