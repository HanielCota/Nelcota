//! Indexed pagination must preserve directory ordering and caller visibility.
mod common;
use common::TestApp;
use nelcota_core::Claims;
use serde_json::{Value, json};

fn service() -> Claims {
    Claims::from_payload(json!({ "role": "service_role" })).unwrap()
}

async fn insert(app: &TestApp, name: &str, owner: uuid::Uuid) {
    app.admin_client.execute(
        "INSERT INTO storage.objects (bucket_id,name,owner,version,size,mime_type,etag) VALUES ('docs',$1,$2,gen_random_uuid(),1,'text/plain','etag')",
        &[&name, &owner],
    ).await.unwrap();
}

#[tokio::test]
async fn directory_pages_preserve_c_order_unicode_literal_prefixes_and_renames() {
    let app = TestApp::spawn().await;
    app.admin_client
        .batch_execute("INSERT INTO storage.buckets (id) VALUES ('docs')")
        .await
        .unwrap();
    let names = [
        "a",
        "a/b/deep/file.txt",
        "a-/file.txt",
        "a./file.txt",
        "a!/file.txt",
        "a b/file.txt",
        "a+/file.txt",
        "100%_done/file.txt",
        "100XYdone/other.txt",
        "é/leaf.txt",
        "é-/leaf.txt",
        "é/nested/leaf.txt",
        "😀/sub/file.txt",
    ];
    for name in names {
        insert(&app, name, app.user_a).await;
    }
    for prefix in ["", "a/", "é/", "😀/", "100%_done/"] {
        let expected_folders: Vec<String> = app.admin_client.query(
            "SELECT DISTINCT split_part(substr(name,length($1::text)+1),'/',1) COLLATE \"C\" AS folder FROM storage.objects WHERE bucket_id='docs' AND starts_with(name,$1) AND strpos(substr(name,length($1)+1),'/')>0 ORDER BY folder",
            &[&prefix],
        ).await.unwrap().iter().map(|row| row.get(0)).collect();
        let expected_files: Vec<String> = names
            .iter()
            .filter(|name| name.starts_with(prefix) && !name[prefix.len()..].contains('/'))
            .map(|name| name.to_string())
            .collect();
        let all = app
            .storage
            .list(&service(), "docs", prefix, Some(100), 0)
            .await
            .unwrap();
        assert_eq!(all.folders, expected_folders, "prefix {prefix}");
        let mut file_names: Vec<String> =
            all.objects.into_iter().map(|object| object.name).collect();
        file_names.sort();
        let mut expected_files = expected_files;
        expected_files.sort();
        assert_eq!(file_names, expected_files, "prefix {prefix}");
        for offset in 0..expected_folders.len() + 1 {
            let page = app
                .storage
                .list(&service(), "docs", prefix, Some(2), offset as i64)
                .await
                .unwrap();
            assert_eq!(
                page.folders,
                expected_folders
                    .iter()
                    .skip(offset)
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>()
            );
        }
    }
    app.admin_client
        .batch_execute("UPDATE storage.objects SET name='moved/leaf.txt' WHERE name='é/leaf.txt'")
        .await
        .unwrap();
    let old = app
        .storage
        .list(&service(), "docs", "é/", Some(100), 0)
        .await
        .unwrap();
    assert!(old.objects.is_empty());
    let moved = app
        .storage
        .list(&service(), "docs", "moved/", Some(100), 0)
        .await
        .unwrap();
    assert_eq!(moved.objects[0].name, "moved/leaf.txt");
}

#[tokio::test]
async fn directory_seeks_apply_rls_before_folder_selection_and_offsets() {
    let app = TestApp::spawn().await;
    app.admin_client.batch_execute("INSERT INTO storage.buckets (id) VALUES ('docs');
        CREATE POLICY own_files ON storage.objects FOR SELECT TO authenticated USING (owner=auth.uid());").await.unwrap();
    for (name, owner) in [
        ("shared/a/deeper/hidden.txt", app.user_b),
        ("shared/a/visible.txt", app.user_a),
        ("shared/a-/hidden.txt", app.user_b),
        ("shared/b/visible.txt", app.user_a),
        ("shared/c/hidden.txt", app.user_b),
        ("shared/d/visible.txt", app.user_a),
        ("shared/root.txt", app.user_a),
    ] {
        insert(&app, name, owner).await;
    }
    let a = Claims::from_payload(json!({ "role": "authenticated", "sub": app.user_a })).unwrap();
    let first = app
        .storage
        .list(&a, "docs", "shared/", Some(2), 0)
        .await
        .unwrap();
    assert_eq!(first.folders, ["a", "b"]);
    assert_eq!(first.objects.len(), 1);
    assert_eq!(first.objects[0].name, "shared/root.txt");
    let next = app
        .storage
        .list(&a, "docs", "shared/", Some(2), 2)
        .await
        .unwrap();
    assert_eq!(next.folders, ["d"]);
    assert!(next.objects.is_empty());
    let private = app
        .storage
        .list(&a, "docs", "shared/c/", Some(2), 0)
        .await
        .unwrap();
    assert!(private.folders.is_empty() && private.objects.is_empty());
    let visitor = app
        .storage
        .list(&Claims::anon(), "docs", "shared/", Some(2), 0)
        .await
        .unwrap();
    assert!(visitor.folders.is_empty() && visitor.objects.is_empty());
}

fn visited_objects(plan: &Value) -> u64 {
    let mut rows = 0;
    if plan["Relation Name"] == "objects" {
        rows = (plan["Actual Rows"].as_u64().unwrap_or(0)
            + plan["Rows Removed by Filter"].as_u64().unwrap_or(0))
            * plan["Actual Loops"].as_u64().unwrap_or(1);
    }
    if let Some(children) = plan["Plans"].as_array() {
        rows += children.iter().map(visited_objects).sum::<u64>();
    }
    rows
}

#[tokio::test]
async fn file_index_collisions_preserve_exact_scope_and_rls() {
    let app = TestApp::spawn().await;
    // Force every scope into the same index range instead of relying on a
    // particular PostgreSQL hash implementation to produce a collision.
    app.admin_client.batch_execute(
        "CREATE OR REPLACE FUNCTION storage.directory_hash(bucket_id text, parent_key bytea)
         RETURNS integer LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE SET search_path=pg_catalog AS $$ SELECT 0 $$;
         INSERT INTO storage.buckets (id) VALUES ('docs'),('other');
         CREATE POLICY own_docs ON storage.objects FOR SELECT TO authenticated
             USING (bucket_id='docs' AND owner=auth.uid());"
    ).await.unwrap();
    for (name, owner) in [
        ("root-a.txt", app.user_a),
        ("root-private.txt", app.user_b),
        ("parent/file.txt", app.user_a),
        ("parent/sub/hidden.txt", app.user_b),
        ("parent/sub/visible.txt", app.user_a),
    ] {
        insert(&app, name, owner).await;
    }
    app.admin_client
        .execute(
            "INSERT INTO storage.objects (bucket_id,name,owner,version,size,mime_type,etag)
         VALUES ('other','root-other.txt',$1,gen_random_uuid(),1,'text/plain','etag')",
            &[&app.user_a],
        )
        .await
        .unwrap();
    let caller = Claims::from_payload(json!({"role":"authenticated","sub":app.user_a})).unwrap();
    let root = app
        .storage
        .list(&caller, "docs", "", Some(100), 0)
        .await
        .unwrap();
    assert_eq!(root.folders, ["parent"]);
    assert_eq!(
        root.objects
            .iter()
            .map(|o| o.name.as_str())
            .collect::<Vec<_>>(),
        ["root-a.txt"]
    );
    let nested = app
        .storage
        .list(&caller, "docs", "parent/", Some(100), 0)
        .await
        .unwrap();
    assert_eq!(nested.folders, ["sub"]);
    assert_eq!(
        nested
            .objects
            .iter()
            .map(|o| o.name.as_str())
            .collect::<Vec<_>>(),
        ["parent/file.txt"]
    );
    assert!(
        app.storage
            .list(&caller, "docs", "", Some(1), 1)
            .await
            .unwrap()
            .objects
            .is_empty()
    );
    let other = app
        .storage
        .list(&service(), "other", "", Some(100), 0)
        .await
        .unwrap();
    assert_eq!(other.objects.len(), 1);
    assert_eq!(other.objects[0].name, "root-other.txt");
    let hidden = app
        .storage
        .list(&Claims::anon(), "docs", "", Some(100), 0)
        .await
        .unwrap();
    assert!(hidden.folders.is_empty() && hidden.objects.is_empty());
}

#[tokio::test]
async fn directory_indexes_accept_legacy_multibyte_sql_names() {
    let app = TestApp::spawn().await;
    app.admin_client
        .batch_execute("INSERT INTO storage.buckets (id) VALUES ('docs')")
        .await
        .unwrap();
    let parent: String = (20_000..20_800)
        .map(|point| char::from_u32(point).unwrap())
        .collect();
    let name = format!("{parent}/leaf.txt");
    assert!(name.len() > 1024 && name.chars().count() <= 1024);
    insert(&app, &name, app.user_a).await;
    let key: Vec<u8> = app
        .admin_client
        .query_one(
            "SELECT directory_key FROM storage.objects WHERE bucket_id='docs' AND name=$1",
            &[&name],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(key, [parent.as_bytes(), &[0]].concat());
}

#[tokio::test]
async fn a_folder_page_seeks_one_row_per_folder_instead_of_aggregating_the_bucket() {
    let app = TestApp::spawn().await;
    app.admin_client.batch_execute("INSERT INTO storage.buckets (id) VALUES ('docs');
        INSERT INTO storage.objects (bucket_id,name,version,size,mime_type,etag)
        SELECT 'docs','folder'||lpad(n::text,6,'0')||'/file.txt',gen_random_uuid(),1,'text/plain','etag' FROM generate_series(1,100000) n;
        ANALYZE storage.objects;").await.unwrap();
    let page = app
        .storage
        .list(&service(), "docs", "", Some(101), 0)
        .await
        .unwrap();
    assert_eq!(page.folders.len(), 101);
    assert!(page.objects.is_empty());
    let sql = format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {}",
        include_str!("../../storage/src/list.sql")
    );
    let empty: Vec<u8> = Vec::new();
    let upper: Option<Vec<u8>> = None;
    let plan: Value = app
        .admin_client
        .query_one(&sql, &[&"docs", &"", &empty, &upper, &101_i64, &0_i64])
        .await
        .unwrap()
        .get(0);
    let visited = visited_objects(&plan[0]["Plan"]);
    assert!(plan.to_string().contains("objects_directory_idx"));
    assert!(
        visited <= 250,
        "a page must not scan all 100000 objects: visited {visited}"
    );
    println!(
        "100000 objects, 101 folders: {} ms; visited {visited} object rows",
        plan[0]["Execution Time"]
    );
}

#[tokio::test]
async fn a_file_page_uses_the_name_index_range_without_scanning_previous_subtrees() {
    let app = TestApp::spawn().await;
    app.admin_client
        .batch_execute(
            "INSERT INTO storage.buckets (id) VALUES ('docs');
        INSERT INTO storage.objects (bucket_id,name,version,size,mime_type,etag)
        SELECT 'docs',prefix||lpad(n::text,6,'0')||'.txt',gen_random_uuid(),1,'text/plain','etag'
        FROM generate_series(1,20000) n CROSS JOIN (VALUES ('a/sub/'),('z/')) prefixes(prefix);
        ANALYZE storage.objects;",
        )
        .await
        .unwrap();
    let page = app
        .storage
        .list(&service(), "docs", "z/", Some(101), 0)
        .await
        .unwrap();
    assert_eq!(page.objects.len(), 101);
    assert!(page.folders.is_empty());
    assert_eq!(page.objects[0].name, "z/000001.txt");
    let sql = format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {}",
        include_str!("../../storage/src/list.sql")
    );
    let key = vec![b'z', 0];
    let upper = Some(vec![b'z', 1]);
    let plan: Value = app
        .admin_client
        .query_one(&sql, &[&"docs", &"z/", &key, &upper, &101_i64, &0_i64])
        .await
        .unwrap()
        .get(0);
    let visited = visited_objects(&plan[0]["Plan"]);
    assert!(
        visited <= 250,
        "a page must skip earlier subtrees: visited {visited}"
    );
}

#[tokio::test]
async fn review_generated_directory_keys_preserve_custom_sql_writer_permissions() {
    let app = TestApp::spawn().await;
    let (mut client, connection) = app.admin.connect(tokio_postgres::NoTls).await.unwrap();
    let driver = tokio::spawn(connection);
    let tx = client.transaction().await.unwrap();
    let role = format!("listing_writer_{}", uuid::Uuid::new_v4().simple());
    tx.batch_execute(&format!(
        "CREATE ROLE {role} NOLOGIN NOINHERIT;
         INSERT INTO storage.buckets (id) VALUES ('docs');
         GRANT USAGE ON SCHEMA storage TO {role};
         GRANT INSERT ON storage.objects TO {role};
         CREATE POLICY custom_writer ON storage.objects FOR INSERT TO {role} WITH CHECK (bucket_id='docs');
         SET LOCAL ROLE {role};"
    )).await.unwrap();
    let inserted = tx
        .execute(
            "INSERT INTO storage.objects (bucket_id,name,owner,version,size,mime_type,etag)
        VALUES ('docs','parent/file.txt',NULL,gen_random_uuid(),1,'text/plain','etag')",
            &[],
        )
        .await;
    tx.rollback().await.unwrap(); // Also removes the temporary cluster role.
    drop(client);
    driver.await.unwrap().unwrap();
    assert!(
        inserted.is_ok(),
        "existing custom SQL writers must not need a new privilege: {inserted:?}"
    );
}

#[tokio::test]
async fn review_wide_file_pages_do_not_require_an_analyze_first() {
    let app = TestApp::spawn_with(common::Options {
        pool_size: 1,
        ..common::Options::default()
    })
    .await;
    app.admin_client.batch_execute("ALTER TABLE storage.objects SET (autovacuum_enabled=false);
        INSERT INTO storage.buckets (id) VALUES ('docs');
        INSERT INTO storage.objects (bucket_id,name,version,size,mime_type,etag)
        SELECT 'docs',lpad(n::text,6,'0')||'.txt',gen_random_uuid(),1,'text/plain','etag' FROM generate_series(1,50000) n;").await.unwrap();
    app.admin_client
        .execute("UPDATE storage.objects SET owner=$1", &[&app.user_a])
        .await
        .unwrap();
    app.admin_client.batch_execute("CREATE POLICY own_root_files ON storage.objects FOR SELECT TO authenticated USING (owner=auth.uid());").await.unwrap();
    let sql = format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {}",
        include_str!("../../storage/src/list.sql")
    );
    let empty: Vec<u8> = Vec::new();
    let upper: Option<Vec<u8>> = None;
    let page = app
        .storage
        .list(&service(), "docs", "", Some(101), 0)
        .await
        .unwrap();
    assert_eq!(page.objects.len(), 101);
    assert_eq!(page.objects[0].name, "000001.txt");
    let reused = app.pool.get().await.unwrap();
    assert_eq!(
        reused
            .query_one("SHOW enable_bitmapscan", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "on"
    );
    assert_eq!(
        reused
            .query_one("SELECT current_user::text", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "authenticator"
    );
    drop(reused);
    let mut client = app.pool.get().await.unwrap();
    let caller = Claims::from_payload(json!({"role":"authenticated","sub":app.user_a})).unwrap();
    let tx = nelcota_core::db::begin_request(&mut client, &caller)
        .await
        .unwrap();
    tx.batch_execute("SET LOCAL enable_bitmapscan = off")
        .await
        .unwrap();
    let plan: Value = tx
        .query_one(&sql, &[&"docs", &"", &empty, &upper, &101_i64, &0_i64])
        .await
        .unwrap()
        .get(0);
    tx.rollback().await.unwrap();
    assert_eq!(
        client
            .query_one("SHOW enable_bitmapscan", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "on"
    );
    drop(client);
    let visited = visited_objects(&plan[0]["Plan"]);
    println!(
        "Fresh root-file page: {} ms; visited {visited} rows",
        plan[0]["Execution Time"]
    );
    assert!(
        visited <= 250,
        "file pagination must remain indexed before ANALYZE: visited {visited}; plan {plan}"
    );
}
