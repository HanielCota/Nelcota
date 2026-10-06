//! Testes de integração do fluxo JWT → role → RLS, contra um Postgres 17 real
//! (testcontainers). Nada de mock do banco: o que está sob teste é o RLS.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use jsonwebtoken::get_current_timestamp;
use nelcota_core::{Claims, db};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

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
    let app = TestApp::spawn_with(Options {
        pool_size: 1,
        ..Options::default()
    })
    .await;
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
    let app = TestApp::spawn_with(Options {
        pool_size: 1,
        ..Options::default()
    })
    .await;
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
