//! Testes de integração do fluxo JWT → role → RLS, contra um Postgres 17 real
//! (testcontainers). Nada de mock do banco: o que está sob teste é o RLS.
//!
//! Requer Docker em execução.

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use jsonwebtoken::{EncodingKey, Header, get_current_timestamp};
use nelcota_auth::Hs256Verifier;
use nelcota_core::{Claims, db};
use nelcota_server::{AppState, app};
use serde_json::{Value, json};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use tower::ServiceExt;
use uuid::Uuid;

const JWT_SECRET: &str = "segredo-de-teste-com-mais-de-32-caracteres";
const AUTHENTICATOR_PASSWORD: &str = "senha-do-authenticator-de-teste";

struct TestApp {
    router: Router,
    pool: deadpool_postgres::Pool,
    admin: tokio_postgres::Config,
    user_a: Uuid,
    user_b: Uuid,
    _container: ContainerAsync<Postgres>,
}

impl TestApp {
    async fn spawn() -> Self {
        Self::spawn_with_pool(4).await
    }

    async fn spawn_with_pool(pool_size: usize) -> Self {
        let container = Postgres::default()
            .with_tag("17-alpine")
            .with_cmd([
                "postgres",
                "-c",
                "fsync=off",
                "-c",
                "shared_preload_libraries=pg_stat_statements",
            ])
            .start()
            .await
            .expect("Docker precisa estar rodando para os testes de integração");
        let host = container.get_host().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let admin: tokio_postgres::Config =
            format!("postgres://postgres:postgres@{host}:{port}/postgres")
                .parse()
                .unwrap();

        db::bootstrap(&admin, AUTHENTICATOR_PASSWORD).await.unwrap();

        // Fixture: tabela de exemplo + uma linha para cada usuário, inseridas
        // como superusuário (que ignora RLS).
        let (user_a, user_b) = (Uuid::new_v4(), Uuid::new_v4());
        let (client, connection) = admin.connect(tokio_postgres::NoTls).await.unwrap();
        tokio::spawn(connection);
        client
            .batch_execute(include_str!("../../../examples/todos.sql"))
            .await
            .unwrap();
        client
            .execute(
                "INSERT INTO public.todos (user_id, title) VALUES ($1, 'tarefa de A'), ($2, 'tarefa de B')",
                &[&user_a, &user_b],
            )
            .await
            .unwrap();

        let pool = db::api_pool(&admin, AUTHENTICATOR_PASSWORD, pool_size);
        let state = AppState {
            pool: pool.clone(),
            verifier: Arc::new(Hs256Verifier::new(JWT_SECRET.as_bytes())),
        };
        TestApp {
            router: app(state, Duration::from_secs(5)),
            pool,
            admin,
            user_a,
            user_b,
            _container: container,
        }
    }

    async fn get(&self, path: &str, token: Option<&str>) -> (StatusCode, Value) {
        let mut request = Request::get(path);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let response = self
            .router
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }
}

fn token(claims: Value) -> String {
    token_with_secret(claims, JWT_SECRET)
}

fn token_with_secret(mut claims: Value, secret: &str) -> String {
    if claims.get("exp").is_none() {
        claims["exp"] = json!(get_current_timestamp() + 3600);
    }
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

fn user_token(sub: Uuid) -> String {
    token(json!({ "role": "authenticated", "sub": sub }))
}

fn titles(body: &Value) -> Vec<&str> {
    body.as_array()
        .expect("resposta deveria ser um array")
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn usuario_so_le_os_proprios_dados() {
    let app = TestApp::spawn().await;

    let (status, body) = app
        .get("/rest/v1/todos", Some(&user_token(app.user_a)))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), ["tarefa de A"]);

    let (status, body) = app
        .get("/rest/v1/todos", Some(&user_token(app.user_b)))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), ["tarefa de B"]);

    // Usuário sem nenhuma linha recebe lista vazia, não erro.
    let (status, body) = app
        .get("/rest/v1/todos", Some(&user_token(Uuid::new_v4())))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
}

#[tokio::test]
async fn anon_nao_acessa_tabela_protegida() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rest/v1/todos", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "db_error");

    // Mesmo com um token anon explícito.
    let (status, _) = app
        .get("/rest/v1/todos", Some(&token(json!({ "role": "anon" }))))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn service_role_ignora_rls() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get(
            "/rest/v1/todos",
            Some(&token(json!({ "role": "service_role" }))),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), ["tarefa de A", "tarefa de B"]);
}

#[tokio::test]
async fn role_invalida_no_jwt_e_rejeitada() {
    let app = TestApp::spawn().await;
    let sub = app.user_a;
    for claims in [
        json!({ "role": "postgres", "sub": sub }),
        json!({ "role": "authenticator", "sub": sub }),
        json!({ "role": "pg_read_all_data", "sub": sub }),
        json!({ "sub": sub }),
        // authenticated exige `sub` em formato uuid.
        json!({ "role": "authenticated" }),
        json!({ "role": "authenticated", "sub": "nao-e-uuid" }),
    ] {
        let (status, body) = app
            .get("/rest/v1/todos", Some(&token(claims.clone())))
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "claims {claims}");
        assert_eq!(body["code"], "invalid_token");
    }
}

#[tokio::test]
async fn jwt_expirado_ou_com_assinatura_invalida_retorna_401() {
    let app = TestApp::spawn().await;
    let sub = app.user_a;

    let expirado = token(json!({
        "role": "authenticated",
        "sub": sub,
        "exp": get_current_timestamp() - 3600,
    }));
    let assinatura_errada = token_with_secret(
        json!({ "role": "authenticated", "sub": sub }),
        "outro-segredo-qualquer-com-32-caracteres!",
    );
    // Payload adulterado: troca o `sub` mantendo a assinatura original.
    let adulterado = {
        let original = user_token(app.user_b);
        let parts: Vec<&str> = original.split('.').collect();
        let forjado = user_token(app.user_a);
        let payload_a = forjado.split('.').nth(1).unwrap();
        format!("{}.{}.{}", parts[0], payload_a, parts[2])
    };
    // `alg: none` (cabeçalho {"alg":"none","typ":"JWT"}).
    let alg_none = format!(
        "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.{}.",
        user_token(sub).split('.').nth(1).unwrap()
    );

    for (caso, token) in [
        ("expirado", expirado.as_str()),
        ("assinatura errada", &assinatura_errada),
        ("payload adulterado", &adulterado),
        ("alg none", &alg_none),
        ("lixo", "isto.nao.e-um-jwt"),
    ] {
        let (status, body) = app.get("/rest/v1/todos", Some(token)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{caso}");
        assert_eq!(body["code"], "invalid_token", "{caso}");
    }

    // Esquema diferente de Bearer também é rejeitado (não vira anon).
    let response = app
        .router
        .clone()
        .oneshot(
            Request::get("/rest/v1/todos")
                .header(header::AUTHORIZATION, "Basic dXNlcjpzZW5oYQ==")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().contains_key(header::WWW_AUTHENTICATE));
}

/// Com uma única conexão no pool, a role/claims de um request não podem
/// vazar para o seguinte.
#[tokio::test]
async fn role_e_claims_nao_vazam_entre_requests_do_pool() {
    let app = TestApp::spawn_with_pool(1).await;
    let service = token(json!({ "role": "service_role" }));

    let (status, body) = app.get("/rest/v1/todos", Some(&service)).await;
    assert_eq!((status, titles(&body).len()), (StatusCode::OK, 2));

    let (status, _) = app.get("/rest/v1/todos", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = app
        .get("/rest/v1/todos", Some(&user_token(app.user_a)))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), ["tarefa de A"]);

    let (status, body) = app
        .get("/rest/v1/todos", Some(&user_token(app.user_b)))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), ["tarefa de B"]);

    // Fora de uma transação de request, a conexão volta a ser só `authenticator`.
    let client = app.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT current_user::text, coalesce(current_setting('request.jwt.claims', true), '')",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "authenticator");
    assert_eq!(row.get::<_, String>(1), "");
}

/// Transação abortada (erro no meio do request) também não deixa resíduo.
#[tokio::test]
async fn transacao_com_erro_faz_rollback_da_role() {
    let app = TestApp::spawn_with_pool(1).await;
    let claims = Claims::from_payload(json!({ "role": "service_role" })).unwrap();
    {
        let mut client = app.pool.get().await.unwrap();
        let tx = db::begin_request(&mut client, &claims).await.unwrap();
        assert!(tx.execute("SELECT 1/0", &[]).await.is_err());
        // drop sem commit
    }
    let client = app.pool.get().await.unwrap();
    let user: String = client
        .query_one("SELECT current_user::text", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(user, "authenticator");
}

/// As policies valem também para escrita: A não insere linha em nome de B, e
/// `auth.uid()`/`auth.role()` refletem o JWT dentro da transação.
#[tokio::test]
async fn rls_bloqueia_escrita_em_nome_de_outro_usuario() {
    let app = TestApp::spawn().await;
    let claims =
        Claims::from_payload(json!({ "role": "authenticated", "sub": app.user_a })).unwrap();

    let mut client = app.pool.get().await.unwrap();
    let tx = db::begin_request(&mut client, &claims).await.unwrap();
    let row = tx
        .query_one("SELECT auth.uid(), auth.role(), current_user::text", &[])
        .await
        .unwrap();
    assert_eq!(row.get::<_, Uuid>(0), app.user_a);
    assert_eq!(row.get::<_, String>(1), "authenticated");
    assert_eq!(row.get::<_, String>(2), "authenticated");

    let err = tx
        .execute(
            "INSERT INTO public.todos (user_id, title) VALUES ($1, 'invasão')",
            &[&app.user_b],
        )
        .await
        .unwrap_err();
    assert_eq!(
        err.code(),
        Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE)
    );
}

#[tokio::test]
async fn health_responde_ok() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn bootstrap_e_idempotente() {
    let app = TestApp::spawn().await;
    let client = app.pool.get().await.unwrap();
    // `authenticator` não consegue ler a tabela de controle das migrações.
    assert!(
        client
            .query("SELECT * FROM nelcota.schema_migrations", &[])
            .await
            .is_err()
    );
    drop(client);

    db::bootstrap(&app.admin, AUTHENTICATOR_PASSWORD)
        .await
        .unwrap();
    db::bootstrap(&app.admin, AUTHENTICATOR_PASSWORD)
        .await
        .unwrap();

    let (status, _) = app.get("/health", None).await;
    assert_eq!(status, StatusCode::OK);
}
