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

/// Percent-encoding para pôr JSON na query string.
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
        .map(|r| r["nome"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn filtros_na_grade_validados_pelo_catalogo() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    let table = |filters: Value| {
        format!(
            "/admin/api/tables/produtos?sort=id&filters={}",
            enc(&filters.to_string())
        )
    };

    let caros = get(
        &app,
        &table(json!([{ "column": "preco", "op": "gte", "value": "10" }])),
        &cookie,
    )
    .await;
    assert_eq!(caros.status, StatusCode::OK, "{}", caros.text);
    assert_eq!(names(&caros.body), ["Caderno", "Mochila"]);
    // O total acompanha o filtro (e é exato).
    assert_eq!(caros.body["total"], 2);
    assert_eq!(caros.body["total_exact"], true);

    let contem = get(
        &app,
        &table(json!([{ "column": "nome", "op": "ilike", "value": "*CA*" }])),
        &cookie,
    )
    .await
    .body;
    assert_eq!(names(&contem), ["Caneta", "Caderno"]);

    let negado = get(
        &app,
        &table(json!([
            { "column": "estoque", "op": "eq", "value": "0", "not": true },
            { "column": "preco", "op": "lt", "value": "10" },
        ])),
        &cookie,
    )
    .await
    .body;
    assert_eq!(names(&negado), ["Caneta", "Régua, 30cm"]);

    // Valor do tipo errado: 400 com a mensagem do Postgres, não 500.
    let tipo = get(
        &app,
        &table(json!([{ "column": "preco", "op": "eq", "value": "abc" }])),
        &cookie,
    )
    .await;
    assert_eq!(tipo.status, StatusCode::BAD_REQUEST, "{}", tipo.text);
    assert!(tipo.text.contains("numeric"), "{}", tipo.text);

    // Coluna fora do catálogo e operador fora da lista: recusados antes do banco.
    for filters in [
        json!([{ "column": "nao_existe", "op": "eq", "value": "1" }]),
        json!([{ "column": "id", "op": "in", "value": "(1,2)" }]),
    ] {
        let reply = get(&app, &table(filters), &cookie).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{}", reply.text);
    }
    let lixo = get(
        &app,
        "/admin/api/tables/produtos?filters=nao-e-json",
        &cookie,
    )
    .await;
    assert_eq!(lixo.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn exportar_tabela_em_csv_e_json() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let csv = get(
        &app,
        "/admin/api/tables/produtos/export?format=csv&sort=id",
        &cookie,
    )
    .await;
    assert_eq!(csv.status, StatusCode::OK, "{}", csv.text);
    assert_eq!(csv.headers[header::CONTENT_TYPE], "text/csv; charset=utf-8");
    assert_eq!(
        csv.headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"produtos.csv\""
    );
    let lines: Vec<&str> = csv.text.split("\r\n").collect();
    assert_eq!(lines[0], "\u{feff}id,nome,preco,estoque,criado_em,slug");
    assert!(lines[1].starts_with("1,Caneta,2.50,100,"), "{}", lines[1]);
    // Vírgula dentro do valor: campo entre aspas.
    assert!(
        lines[4].starts_with("4,\"Régua, 30cm\",4.00,"),
        "{}",
        lines[4]
    );
    assert_eq!(lines.len(), 6, "cabeçalho + 4 linhas + final vazio");

    // A exportação segue os filtros da grade.
    let filters = enc(&json!([{ "column": "estoque", "op": "eq", "value": "0" }]).to_string());
    let filtrado = get(
        &app,
        &format!("/admin/api/tables/produtos/export?format=json&filters={filters}"),
        &cookie,
    )
    .await;
    assert_eq!(filtrado.headers[header::CONTENT_TYPE], "application/json");
    let rows: Value = serde_json::from_str(&filtrado.text).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["nome"], "Mochila");
    // numeric sai com o texto do Postgres.
    assert!(filtrado.text.contains("120.00"), "{}", filtrado.text);

    let erro = get(
        &app,
        &format!(
            "/admin/api/tables/produtos/export?format=csv&filters={}",
            enc(&json!([{ "column": "id", "op": "eq", "value": "x" }]).to_string())
        ),
        &cookie,
    )
    .await;
    assert_eq!(erro.status, StatusCode::BAD_REQUEST, "{}", erro.text);
    let formato = get(
        &app,
        "/admin/api/tables/produtos/export?format=xls",
        &cookie,
    )
    .await;
    assert_eq!(formato.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn colunas_indicam_a_chave_estrangeira() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    app.admin_client
        .batch_execute(
            "CREATE TABLE public.avaliacoes (
                 id int PRIMARY KEY,
                 produto_id int REFERENCES public.produtos (id),
                 nota int
             );
             GRANT SELECT ON public.avaliacoes TO service_role;",
        )
        .await
        .unwrap();

    // O catálogo recarrega sozinho depois do DDL.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let data = loop {
        let reply = get(&app, "/admin/api/tables/avaliacoes", &cookie).await;
        if reply.status == StatusCode::OK {
            break reply.body;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "tabela nova não apareceu"
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
        column("produto_id")["references"],
        json!({ "table": "produtos", "column": "id" })
    );
    assert_eq!(column("nota")["references"], Value::Null);
}

#[tokio::test]
async fn estrutura_da_tabela_lida_do_catalogo_do_postgres() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let reply = get(&app, "/admin/api/tables/produtos/structure", &cookie).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let data = reply.body;
    assert_eq!(data["primary_key"], json!(["id"]));
    assert_eq!(data["rls_enabled"], false);
    assert_eq!(data["comment"], "Catálogo de produtos");
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
    assert_eq!(column("nome")["data_type"], "character varying(80)");
    assert_eq!(column("nome")["nullable"], false);
    assert_eq!(column("estoque")["default"], "0");
    assert_eq!(column("criado_em")["default"], "now()");
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
            "CREATE TABLE public.itens (
                 id int PRIMARY KEY,
                 produto_id int REFERENCES public.produtos (id) ON DELETE CASCADE,
                 codigo text CONSTRAINT itens_codigo_key UNIQUE
             );",
        )
        .await
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let itens = loop {
        let reply = get(&app, "/admin/api/tables/itens/structure", &cookie).await;
        if reply.status == StatusCode::OK {
            break reply.body;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "tabela nova não apareceu"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    let columns = itens["columns"].as_array().unwrap();
    assert_eq!(
        columns[1]["references"],
        json!({ "table": "produtos", "column": "id", "on_delete": "cascade", "constraint": "itens_produto_id_fkey" })
    );
    assert_eq!(columns[2]["unique"], "itens_codigo_key");

    let missing = get(&app, "/admin/api/tables/nao_existe/structure", &cookie).await;
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
async fn criar_alterar_e_apagar_tabelas_pelo_painel() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    let notas = json!({
        "name": "notas",
        "comment": "Notas da equipe",
        "columns": [
            { "name": "id", "data_type": "bigint", "primary_key": true, "identity": true },
            { "name": "texto", "data_type": "text", "nullable": false },
            { "name": "produto_id", "data_type": "integer",
              "references": { "table": "produtos", "column": "id", "on_delete": "cascade" } },
            { "name": "criada_em", "data_type": "timestamptz", "nullable": false, "default": "now()" },
        ],
        "grants": [{ "role": "anon", "privileges": ["select"] }],
    });

    // Prévia: devolve o SQL e não cria nada.
    let preview = send(
        &app,
        Method::POST,
        "/admin/api/tables",
        &cookie,
        json!({ "table": notas, "preview": true }),
    )
    .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.text);
    assert!(
        preview.body["sql"][0]
            .as_str()
            .unwrap()
            .starts_with("CREATE TABLE \"public\".\"notas\"")
    );
    assert_eq!(
        get(&app, "/admin/api/tables/notas", &cookie).await.status,
        StatusCode::NOT_FOUND
    );

    let created = send(
        &app,
        Method::POST,
        "/admin/api/tables",
        &cookie,
        json!({ "table": notas }),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text);
    // O catálogo já recarregou: a tabela está no painel e na API REST, sem espera.
    let structure = get(&app, "/admin/api/tables/notas/structure", &cookie)
        .await
        .body;
    assert_eq!(structure["rls_enabled"], true);
    assert_eq!(
        structure["grants"][0],
        json!({ "role": "anon", "privileges": ["select"] })
    );
    let rest = app
        .raw(Method::GET, "/rest/v1/notas", &[], String::new())
        .await;
    assert_eq!(rest.status, StatusCode::OK, "{}", rest.text);
    assert_eq!(
        rest.body,
        json!([]),
        "RLS ligado e sem policies: anon não vê nada"
    );

    // Várias alterações numa transação.
    let altered = send(&app, Method::PATCH, "/admin/api/tables/notas", &cookie, json!({ "actions": [
        { "action": "add_column", "column": { "name": "votos", "data_type": "integer", "default": "0", "nullable": false } },
        { "action": "rename_column", "from": "texto", "to": "conteudo" },
        { "action": "set_type", "column": "conteudo", "data_type": "varchar(500)" },
        { "action": "set_unique", "column": "conteudo", "unique": true },
        { "action": "set_grants", "grant": { "role": "authenticated", "privileges": ["select", "insert"] } },
    ] })).await;
    assert_eq!(altered.status, StatusCode::OK, "{}", altered.text);
    let structure = get(&app, "/admin/api/tables/notas/structure", &cookie)
        .await
        .body;
    assert_eq!(
        columns_of(&structure),
        ["id", "conteudo", "produto_id", "criada_em", "votos"]
    );
    let conteudo = &structure["columns"][1];
    assert_eq!(conteudo["data_type"], "character varying(500)");
    assert!(conteudo["unique"].is_string());
    assert_eq!(
        structure["grants"][1]["privileges"],
        json!(["select", "insert"])
    );

    // Atomicidade: a segunda ação falha (cast impossível) e a primeira não fica.
    let failed = send(
        &app,
        Method::PATCH,
        "/admin/api/tables/notas",
        &cookie,
        json!({ "actions": [
        { "action": "add_column", "column": { "name": "rascunho", "data_type": "boolean" } },
        { "action": "set_type", "column": "criada_em", "data_type": "integer" },
    ] }),
    )
    .await;
    assert_eq!(failed.status, StatusCode::BAD_REQUEST, "{}", failed.text);
    let structure = get(&app, "/admin/api/tables/notas/structure", &cookie)
        .await
        .body;
    assert!(!columns_of(&structure).contains(&"rascunho".to_owned()));

    // Expressão com um segundo comando: o protocolo estendido recusa.
    let injection = send(
        &app,
        Method::PATCH,
        "/admin/api/tables/notas",
        &cookie,
        json!({ "actions": [
        { "action": "add_column", "column": { "name": "x", "data_type": "text",
          "default": "'a'); DROP TABLE public.produtos; --" } },
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
        get(&app, "/admin/api/tables/produtos", &cookie)
            .await
            .status,
        StatusCode::OK
    );

    // Validação antes do banco.
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
    assert!(
        bad_type.text.contains("tipo desconhecido"),
        "{}",
        bad_type.text
    );

    // produtos é referenciada por notas: sem CASCADE o Postgres recusa.
    let blocked = app
        .raw(
            Method::DELETE,
            "/admin/api/tables/produtos",
            &[("cookie", cookie.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(blocked.status, StatusCode::BAD_REQUEST, "{}", blocked.text);
    let dropped = app
        .raw(
            Method::DELETE,
            "/admin/api/tables/notas",
            &[("cookie", cookie.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(dropped.status, StatusCode::OK, "{}", dropped.text);
    assert_eq!(
        get(&app, "/admin/api/tables/notas", &cookie).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.raw(Method::GET, "/rest/v1/notas", &[], String::new())
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
    assert_eq!(types["enums"], json!(["prioridade"]));
}

async fn anon_rows(app: &TestApp) -> usize {
    let reply = app
        .raw(Method::GET, "/rest/v1/produtos", &[], String::new())
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    reply.body.as_array().unwrap().len()
}

#[tokio::test]
async fn policies_criadas_editadas_e_apagadas_pelo_painel() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    assert_eq!(anon_rows(&app).await, 4, "sem RLS, anon vê tudo");

    let rls = send(
        &app,
        Method::PATCH,
        "/admin/api/tables/produtos",
        &cookie,
        json!({ "actions": [{ "action": "set_rls", "enabled": true }] }),
    )
    .await;
    assert_eq!(rls.status, StatusCode::OK, "{}", rls.text);
    assert_eq!(anon_rows(&app).await, 0, "RLS sem policies: ninguém vê");

    let policy = json!({ "name": "leitura pública", "command": "select", "roles": ["anon"], "using": "true" });
    let created = send(
        &app,
        Method::POST,
        "/admin/api/tables/produtos/policies",
        &cookie,
        json!({ "policy": policy }),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text);
    assert_eq!(anon_rows(&app).await, 4);

    // Editar: troca a expressão e o nome numa transação (DROP + CREATE).
    let edited = send(&app, Method::PUT, "/admin/api/tables/produtos/policies/leitura%20p%C3%BAblica", &cookie,
        json!({ "policy": { "name": "em estoque", "command": "select", "roles": ["anon"], "using": "estoque > 0" } })).await;
    assert_eq!(edited.status, StatusCode::OK, "{}", edited.text);
    assert_eq!(anon_rows(&app).await, 3, "Mochila tem estoque 0");
    let listed = get(&app, "/admin/api/policies", &cookie).await.body;
    let produtos = listed["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "produtos")
        .unwrap()
        .clone();
    assert_eq!(produtos["policies"][0]["name"], "em estoque");
    assert_eq!(produtos["policies"][0]["using"], "(estoque > 0)");

    // Regras de cada comando validadas antes do banco.
    let invalid = send(
        &app,
        Method::POST,
        "/admin/api/tables/produtos/policies",
        &cookie,
        json!({ "policy": { "name": "x", "command": "insert", "using": "true" } }),
    )
    .await;
    assert_eq!(invalid.status, StatusCode::BAD_REQUEST);
    // Segundo comando escondido na expressão: recusado, e nada muda.
    let injection = send(&app, Method::POST, "/admin/api/tables/produtos/policies", &cookie,
        json!({ "policy": { "name": "y", "command": "select", "using": "true) ; DROP TABLE public.produtos; --" } })).await;
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
            "/admin/api/tables/produtos/policies/em%20estoque",
            &[("cookie", cookie.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(dropped.status, StatusCode::OK, "{}", dropped.text);
    assert_eq!(anon_rows(&app).await, 0);
    let missing = app
        .raw(
            Method::DELETE,
            "/admin/api/tables/produtos/policies/nao-existe",
            &[("cookie", cookie.as_str())],
            String::new(),
        )
        .await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST, "{}", missing.text);
}

#[tokio::test]
async fn token_service_role_emitido_pelo_painel() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let anon = app
        .raw(Method::GET, "/rest/v1/segredos", &[], String::new())
        .await;
    assert_ne!(
        anon.status,
        StatusCode::OK,
        "segredos só tem GRANT para service_role"
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
            "/rest/v1/segredos",
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
    }
    // Sem sessão do painel, nada de token.
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
