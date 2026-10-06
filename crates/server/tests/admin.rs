//! Testes do painel administrativo: login separado, proteção de rotas, CSRF,
//! alerta de tabela sem RLS, editor SQL, edição de tabelas, usuários,
//! policies e escape de HTML.

mod common;

use axum::http::{Method, StatusCode, header};
use common::*;
use serde_json::json;

const FORM: (&str, &str) = ("content-type", "application/x-www-form-urlencoded");

fn form(fields: &[(&str, &str)]) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields)
        .finish()
}

async fn login(app: &TestApp) -> String {
    let reply = app
        .raw(
            Method::POST,
            "/admin/login",
            &[FORM],
            form(&[("email", ADMIN_EMAIL), ("password", ADMIN_PASSWORD)]),
        )
        .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.text);
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

async fn post_form(app: &TestApp, path: &str, cookie: &str, fields: &[(&str, &str)]) -> Reply {
    app.raw(
        Method::POST,
        path,
        &[("cookie", cookie), FORM],
        form(fields),
    )
    .await
}

async fn sql(app: &TestApp, cookie: &str, query: &str) -> serde_json::Value {
    app.raw(
        Method::POST,
        "/admin/sql",
        &[("cookie", cookie), ("content-type", "application/json")],
        json!({ "sql": query }).to_string(),
    )
    .await
    .body
}

#[tokio::test]
async fn login_separado_e_rotas_protegidas() {
    let app = TestApp::spawn().await;

    let reply = app.raw(Method::GET, "/admin/", &[], String::new()).await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    assert_eq!(reply.headers[header::LOCATION], "/admin/login");
    let reply = app
        .raw(
            Method::POST,
            "/admin/sql",
            &[("content-type", "application/json")],
            "{}".into(),
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
                "/admin/login",
                &[FORM],
                form(&[("email", email), ("password", password)]),
            )
            .await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
        assert!(reply.text.contains("Email ou senha inválidos"));
    }

    // Um JWT de usuário (nem de service_role) não abre o painel.
    let reply = app
        .raw(
            Method::GET,
            "/admin/",
            &[("authorization", &format!("Bearer {}", service_token()))],
            String::new(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);

    let cookie = login(&app).await;
    let reply = get(&app, "/admin/", &cookie).await;
    assert_eq!(reply.status, StatusCode::OK);
    let csp = reply.headers[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap();
    assert!(csp.contains("script-src 'self'"));
    assert_eq!(reply.headers[header::X_FRAME_OPTIONS], "DENY");
    // Regressão: com `no-referrer` o navegador manda `Origin: null` no POST do
    // formulário e o login era recusado como CSRF.
    assert_eq!(reply.headers[header::REFERRER_POLICY], "same-origin");
    assert!(reply.text.contains("service_role"));

    // Login como o navegador faz (mesma origem) passa; `Origin: null` não.
    let browser = |origin: &'static str| {
        [
            FORM,
            ("host", "localhost"),
            ("origin", origin),
            ("sec-fetch-site", "same-origin"),
        ]
    };
    let credentials = form(&[("email", ADMIN_EMAIL), ("password", ADMIN_PASSWORD)]);
    let reply = app
        .raw(
            Method::POST,
            "/admin/login",
            &browser("https://localhost"),
            credentials.clone(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    let reply = app
        .raw(Method::POST, "/admin/login", &browser("null"), credentials)
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);

    // Logout invalida a sessão.
    let reply = post_form(&app, "/admin/logout", &cookie, &[]).await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    let reply = get(&app, "/admin/", &cookie).await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn alerta_de_tabela_sem_rls() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let dashboard = get(&app, "/admin/", &cookie).await.text;
    assert!(dashboard.contains("Tabelas expostas sem RLS"));
    assert!(dashboard.contains("<code>produtos</code>"));
    // todos tem RLS + 4 policies; segredos não tem GRANT para anon/authenticated.
    assert!(dashboard.contains("RLS · 4 policies"));
    assert!(!dashboard.contains("<code>segredos</code>"));

    let policies = get(&app, "/admin/policies", &cookie).await.text;
    assert!(policies.contains("todos_dono_select"));
    assert!(policies.contains("(user_id = auth.uid())"));
    assert!(policies.contains("<code>produtos</code> está exposta sem RLS"));
    assert!(policies.contains("Funções executáveis por anon"));

    // Corrigido o problema, o alerta some (o catálogo recarrega sozinho).
    app.admin_client
        .batch_execute("ALTER TABLE public.produtos ENABLE ROW LEVEL SECURITY")
        .await
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let text = get(&app, "/admin/", &cookie).await.text;
        if !text.contains("Tabelas expostas sem RLS") {
            assert!(text.contains("RLS sem policies"));
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
    let reply = app
        .raw(
            Method::POST,
            "/admin/sql",
            &[
                ("cookie", &cookie),
                ("content-type", "application/json"),
                ("origin", "https://site-malicioso.com"),
                ("host", "localhost"),
            ],
            json!({ "sql": "drop table public.todos" }).to_string(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    let reply = app
        .raw(
            Method::POST,
            "/admin/sql",
            &[
                ("cookie", &cookie),
                ("content-type", "application/json"),
                ("sec-fetch-site", "cross-site"),
            ],
            json!({ "sql": "drop table public.todos" }).to_string(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    let out = sql(&app, &cookie, "select count(*) from public.todos").await;
    assert_eq!(out["results"][0]["rows"], json!([["2"]]));
}

#[tokio::test]
async fn editar_tabelas_pelo_painel_com_html_escapado() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let page = get(&app, "/admin/tables/produtos", &cookie).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.text.contains("Caderno"));
    assert!(page.text.contains("numeric(10,2)"));

    // Inserção com tentativa de XSS: fica gravada como texto e é escapada.
    let xss = "<script>alert(1)</script>";
    let reply = post_form(
        &app,
        "/admin/tables/produtos/insert",
        &cookie,
        &[("nome", xss), ("preco", "9.90"), ("estoque", "")],
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    assert!(
        reply.headers[header::LOCATION]
            .to_str()
            .unwrap()
            .contains("ok=")
    );
    let page = get(&app, "/admin/tables/produtos", &cookie).await.text;
    assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!page.contains(xss));

    let id: i32 = app
        .admin_client
        .query_one("SELECT id FROM public.produtos WHERE nome = $1", &[&xss])
        .await
        .unwrap()
        .get(0);
    let estoque: i32 = app
        .admin_client
        .query_one("SELECT estoque FROM public.produtos WHERE id = $1", &[&id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(estoque, 0, "campo vazio na inserção usa o DEFAULT");

    let edit = get(
        &app,
        &format!("/admin/tables/produtos/edit?id={id}"),
        &cookie,
    )
    .await;
    assert_eq!(edit.status, StatusCode::OK);
    assert!(edit.text.contains("value=\"9.90\""));
    let id_text = id.to_string();
    let reply = post_form(
        &app,
        "/admin/tables/produtos/update",
        &cookie,
        &[
            ("__pk__id", &id_text),
            ("nome", "Estojo"),
            ("preco", "12.00"),
            ("estoque", "5"),
        ],
    )
    .await;
    assert!(
        reply.headers[header::LOCATION]
            .to_str()
            .unwrap()
            .contains("ok=")
    );
    let nome: String = app
        .admin_client
        .query_one("SELECT nome FROM public.produtos WHERE id = $1", &[&id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(nome, "Estojo");

    // Erro do banco volta como mensagem na página.
    let reply = post_form(
        &app,
        "/admin/tables/produtos/update",
        &cookie,
        &[("__pk__id", &id_text), ("preco", "-1")],
    )
    .await;
    assert!(
        reply.headers[header::LOCATION]
            .to_str()
            .unwrap()
            .contains("erro=")
    );

    let reply = post_form(
        &app,
        "/admin/tables/produtos/delete",
        &cookie,
        &[("__pk__id", &id_text)],
    )
    .await;
    assert!(
        reply.headers[header::LOCATION]
            .to_str()
            .unwrap()
            .contains("ok=")
    );
    let count: i64 = app
        .admin_client
        .query_one("SELECT count(*) FROM public.produtos WHERE id = $1", &[&id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);

    // Coluna inexistente no formulário é recusada.
    let reply = post_form(
        &app,
        "/admin/tables/produtos/insert",
        &cookie,
        &[
            ("nome", "x"),
            ("preco", "1"),
            ("\"; drop table produtos; --", "1"),
        ],
    )
    .await;
    assert!(
        reply.headers[header::LOCATION]
            .to_str()
            .unwrap()
            .contains("erro=")
    );
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

    let page = get(&app, "/admin/users", &cookie).await.text;
    assert!(page.contains("painel@exemplo.com"));
    assert!(!page.contains("argon2id$"), "hash nunca aparece no painel");

    let reply = post_form(
        &app,
        &format!("/admin/users/{user_id}/revoke"),
        &cookie,
        &[],
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    let refresh = app
        .post(
            "/auth/v1/token?grant_type=refresh_token",
            None,
            json!({ "refresh_token": session["refresh_token"] }),
        )
        .await;
    assert_eq!(refresh.status, StatusCode::BAD_REQUEST);

    let reply = post_form(
        &app,
        &format!("/admin/users/{user_id}/delete"),
        &cookie,
        &[],
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    let page = get(&app, "/admin/users", &cookie).await.text;
    assert!(!page.contains("painel@exemplo.com"));

    let reply = post_form(&app, "/admin/users/nao-e-uuid/delete", &cookie, &[]).await;
    assert_eq!(reply.status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn assets_embutidos() {
    let app = TestApp::spawn().await;
    let reply = app
        .raw(Method::GET, "/admin/assets/admin.js", &[], String::new())
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(
        reply.headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/javascript")
    );
    assert!(reply.text.len() < 8 * 1024, "JS do painel deve ser pequeno");
    let reply = app
        .raw(
            Method::GET,
            "/admin/assets/../Cargo.toml",
            &[],
            String::new(),
        )
        .await;
    assert_ne!(reply.status, StatusCode::OK);
    let login_page = app
        .raw(Method::GET, "/admin/login", &[], String::new())
        .await;
    assert!(login_page.text.contains("type=\"password\""));
}
