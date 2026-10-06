//! Testes do painel administrativo: login separado, API JSON protegida, CSRF,
//! alerta de tabela sem RLS, editor SQL, edição de tabelas, usuários,
//! policies e a SPA embutida.

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
async fn login_separado_e_api_protegida() {
    let app = TestApp::spawn().await;

    for path in [
        "/admin/api/overview",
        "/admin/api/tables/produtos",
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
        (ADMIN_EMAIL, "senha-errada"),
        ("outro@exemplo.com", ADMIN_PASSWORD),
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
        assert_eq!(reply.body["error"], "Email ou senha inválidos.");
    }

    // Um JWT (nem de service_role) não abre o painel.
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

    // Login como o navegador faz (mesma origem) passa; `Origin: null` não.
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

    // Logout invalida a sessão.
    let reply = send(&app, Method::POST, "/admin/api/logout", &cookie, json!({})).await;
    assert_eq!(reply.status, StatusCode::OK);
    let reply = get(&app, "/admin/api/session", &cookie).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn alerta_de_tabela_sem_rls() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let overview = get(&app, "/admin/api/overview", &cookie).await.body;
    assert_eq!(overview["exposed_without_rls"], json!(["produtos"]));
    let table = |name: &str| {
        overview["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .cloned()
            .unwrap()
    };
    assert_eq!(table("produtos")["rls"]["state"], "danger");
    assert_eq!(table("todos")["rls"]["state"], "ok");
    assert_eq!(table("todos")["rls"]["label"], "RLS · 4 policies");
    // segredos não tem GRANT para anon/authenticated: não é "exposta".
    assert_eq!(table("segredos")["rls"]["state"], "none");
    assert_eq!(table("produtos")["grants"]["anon"], json!(["SELECT"]));
    assert_eq!(table("produtos")["rows"], 4);
    assert_eq!(table("produtos")["rows_exact"], true);
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
        .find(|p| p["name"] == "todos_dono_select")
        .unwrap();
    assert_eq!(select["using"], "(user_id = auth.uid())");
    assert_eq!(select["command"], "SELECT");
    assert_eq!(select["roles"], json!(["authenticated"]));
    assert!(
        policies["anon_functions"]
            .as_array()
            .unwrap()
            .contains(&json!("soma"))
    );
    assert!(
        !policies["anon_functions"]
            .as_array()
            .unwrap()
            .contains(&json!("so_servico"))
    );

    // Corrigido o problema, o alerta some (o catálogo recarrega sozinho).
    app.admin_client
        .batch_execute("ALTER TABLE public.produtos ENABLE ROW LEVEL SECURITY")
        .await
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let overview = get(&app, "/admin/api/overview", &cookie).await.body;
        if overview["exposed_without_rls"] == json!([]) {
            let produtos = overview["tables"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["name"] == "produtos")
                .unwrap()
                .clone();
            assert_eq!(produtos["rls"]["state"], "warn");
            break;
        }
        assert!(std::time::Instant::now() < deadline, "alerta não sumiu");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn editor_sql_isolado_e_com_erros_legiveis() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let out = sql(
        &app,
        &cookie,
        "select 1 as um, null as nada; select 'b' as dois",
    )
    .await;
    let results = out["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["columns"], json!(["um", "nada"]));
    assert_eq!(results[0]["rows"], json!([["1", null]]));
    assert_eq!(results[1]["rows"], json!([["b"]]));

    let out = sql(&app, &cookie, "select * from tabela_que_nao_existe").await;
    assert_eq!(out["error"]["code"], "42P01");
    assert!(out["error"]["position"].is_number());

    // Transação aberta e SET ROLE não vazam para a próxima execução.
    sql(&app, &cookie, "BEGIN; SET ROLE anon;").await;
    let out = sql(&app, &cookie, "select current_user::text").await;
    assert_eq!(out["results"][0]["rows"], json!([["postgres"]]));

    // CSRF: origem diferente é recusada.
    for extra in [
        [
            ("origin", "https://site-malicioso.com"),
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

    // Schema para o autocomplete do editor.
    let schema = get(&app, "/admin/api/schema", &cookie).await.body;
    assert!(
        schema["tables"]["produtos"]
            .as_array()
            .unwrap()
            .contains(&json!("preco"))
    );
    assert!(
        schema["tables"]["auth.users"]
            .as_array()
            .unwrap()
            .contains(&json!("email"))
    );
}

#[tokio::test]
async fn editar_tabelas_pelo_painel_com_valores_exatos() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let data = get(&app, "/admin/api/tables/produtos", &cookie).await.body;
    assert_eq!(data["table"]["primary_key"], json!(["id"]));
    assert_eq!(data["table"]["editable"], true);
    let preco = data["table"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "preco")
        .unwrap()
        .clone();
    assert_eq!(preco["full_type"], "numeric(10,2)");
    assert!(
        data["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["nome"] == "Caderno")
    );
    // numeric chega como texto exato (sem passar por f64).
    assert!(
        data["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["preco"] == "15.00")
    );
    assert_eq!(data["total"], 4);

    // Ordenação por coluna (validada contra o catálogo).
    let sorted = get(
        &app,
        "/admin/api/tables/produtos?sort=preco&desc=true",
        &cookie,
    )
    .await
    .body;
    assert_eq!(sorted["rows"][0]["nome"], "Mochila");
    let bad = get(&app, "/admin/api/tables/produtos?sort=preco;drop", &cookie).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);

    // Inserção: texto com cara de HTML é guardado como texto (o escape é do
    // Svelte, que nunca usa {@html}); campo omitido usa o DEFAULT.
    let xss = "<script>alert(1)</script>";
    let reply = send(
        &app,
        Method::POST,
        "/admin/api/tables/produtos/rows",
        &cookie,
        json!({ "values": { "nome": xss, "preco": "9.90" } }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let row = app
        .admin_client
        .query_one(
            "SELECT id, estoque, preco::text FROM public.produtos WHERE nome = $1",
            &[&xss],
        )
        .await
        .unwrap();
    let id: i32 = row.get(0);
    assert_eq!(row.get::<_, i32>(1), 0, "campo omitido usa o DEFAULT");
    assert_eq!(row.get::<_, String>(2), "9.90");

    let reply = send(
        &app,
        Method::PATCH,
        "/admin/api/tables/produtos/rows",
        &cookie,
        json!({ "pk": { "id": id.to_string() }, "values": { "nome": "Estojo", "estoque": "5" } }),
    )
    .await;
    assert_eq!(reply.body["count"], 1, "{}", reply.text);

    // Erro do banco volta como 400 com a mensagem.
    let reply = send(
        &app,
        Method::PATCH,
        "/admin/api/tables/produtos/rows",
        &cookie,
        json!({ "pk": { "id": id.to_string() }, "values": { "preco": "-1" } }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert!(reply.body["error"].as_str().unwrap().contains("check"));

    // Coluna desconhecida ou gerada é recusada.
    for values in [
        json!({ "\"; drop table produtos; --": "1" }),
        json!({ "slug": "x" }),
    ] {
        let reply = send(
            &app,
            Method::POST,
            "/admin/api/tables/produtos/rows",
            &cookie,
            json!({ "values": values }),
        )
        .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    }

    // Apagar várias linhas numa transação.
    let reply = send(
        &app,
        Method::DELETE,
        "/admin/api/tables/produtos/rows",
        &cookie,
        json!({ "pks": [{ "id": id.to_string() }, { "id": "3" }] }),
    )
    .await;
    assert_eq!(reply.body["count"], 2, "{}", reply.text);
    let count: i64 = app
        .admin_client
        .query_one("SELECT count(*) FROM public.produtos", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 3);

    let reply = get(&app, "/admin/api/tables/nao_existe", &cookie).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn usuarios_e_sessoes() {
    let app = TestApp::spawn().await;
    let session = app
        .post(
            "/auth/v1/signup",
            None,
            json!({ "email": "painel@exemplo.com", "password": "senha-forte-123" }),
        )
        .await
        .body;
    let user_id = session["user"]["id"].as_str().unwrap().to_owned();
    let cookie = login(&app).await;

    let users = get(&app, "/admin/api/users?q=painel", &cookie).await;
    assert_eq!(users.body["users"][0]["email"], "painel@exemplo.com");
    assert_eq!(users.body["users"][0]["sessions"], 1);
    assert!(!users.text.contains("argon2id"), "hash nunca sai do banco");

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
    assert!(!users.text.contains("painel@exemplo.com"));

    let reply = send(
        &app,
        Method::DELETE,
        "/admin/api/users/nao-e-uuid",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn spa_embutida_com_cabecalhos_de_seguranca() {
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
        "scripts nunca inline"
    );
    assert_eq!(index.headers[header::X_FRAME_OPTIONS], "DENY");
    // Regressão: com `no-referrer` o navegador manda `Origin: null` nos POSTs.
    assert_eq!(index.headers[header::REFERRER_POLICY], "same-origin");
    assert!(
        !index.text.contains("<script>"),
        "sem script inline no index.html"
    );

    // Rotas do cliente devolvem o mesmo index.html.
    let deep = app
        .raw(Method::GET, "/admin/tables/qualquer", &[], String::new())
        .await;
    assert_eq!(deep.status, StatusCode::OK);
    assert_eq!(deep.text, index.text);

    // O bundle referenciado existe, com cache imutável.
    let script = index
        .text
        .split("src=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("index.html referencia o bundle");
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
        .raw(Method::GET, "/admin/api/nao-existe", &[], String::new())
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

/// Token de handoff assinado com o segredo compartilhado (como outro painel do
/// host faria).
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
async fn login_unico_entre_projetos_do_host() {
    let app = TestApp::spawn().await;

    // A tela de login sabe em qual projeto está.
    let whoami = app
        .raw(Method::GET, "/admin/api/whoami", &[], String::new())
        .await;
    assert_eq!(whoami.body, json!({ "project": "loja", "sso": true }));

    // Token emitido por outro painel do host para "loja": vira sessão.
    let token = handoff_token("loja", ADMIN_EMAIL, 60, SSO_SECRET);
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
        "loja"
    );

    // Uso único.
    assert_eq!(redeem(&app, &token).await.status, StatusCode::UNAUTHORIZED);

    // Destino errado, admin errado, vencido ou segredo errado: recusados.
    for bad in [
        handoff_token("blog", ADMIN_EMAIL, 60, SSO_SECRET),
        handoff_token("loja", "intruso@exemplo.com", 60, SSO_SECRET),
        handoff_token("loja", ADMIN_EMAIL, -120, SSO_SECRET),
        handoff_token(
            "loja",
            ADMIN_EMAIL,
            60,
            "outro-segredo-qualquer-de-32-bytes!!",
        ),
    ] {
        assert_eq!(redeem(&app, &bad).await.status, StatusCode::UNAUTHORIZED);
    }

    // Handoff para outro projeto exige sessão e projeto existente.
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
        json!({ "project": "nada" }),
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
    assert_eq!(base, "https://blog.exemplo.com");
    // O token é para "blog": não serve para entrar na própria "loja".
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
    assert!(ttl <= 60, "token curto");
}

#[tokio::test]
async fn lista_e_estado_dos_projetos_do_host() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let list = get(&app, "/admin/api/projects", &cookie).await.body;
    assert_eq!(list["current"], "loja");
    assert_eq!(list["sso"], true);
    let names: Vec<&str> = list["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["loja", "blog"]);
    assert_eq!(list["projects"][0]["current"], true);
    assert!(
        !list.to_string().contains("secret"),
        "a lista nunca carrega segredos"
    );

    // Fora de um host Docker, os apps não respondem: o estado vem como fora do ar.
    let status = get(&app, "/admin/api/projects/status", &cookie).await.body;
    assert_eq!(status["projects"].as_array().unwrap().len(), 2);
    assert_eq!(status["projects"][1]["healthy"], false);

    let reply = app
        .raw(Method::GET, "/admin/api/projects", &[], String::new())
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}
