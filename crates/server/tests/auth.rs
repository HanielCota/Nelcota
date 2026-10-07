//! Testes do fluxo de autenticação contra Postgres real: cadastro, login,
//! refresh com rotação e detecção de reuso, logout, JWKS, rate limit e
//! recuperação de senha por email.

mod common;

use axum::http::{StatusCode, header};
use common::*;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, jwk::JwkSet};
use nelcota_core::{Claims, db};
use serde_json::{Value, json};
use uuid::Uuid;

async fn signup(app: &TestApp, email: &str, password: &str) -> Reply {
    app.post(
        "/auth/v1/signup",
        None,
        json!({ "email": email, "password": password }),
    )
    .await
}

async fn login(app: &TestApp, email: &str, password: &str) -> Reply {
    app.post(
        "/auth/v1/token?grant_type=password",
        None,
        json!({ "email": email, "password": password }),
    )
    .await
}

async fn refresh(app: &TestApp, refresh_token: &str) -> Reply {
    app.post(
        "/auth/v1/token?grant_type=refresh_token",
        None,
        json!({ "refresh_token": refresh_token }),
    )
    .await
}

fn str_field<'a>(body: &'a Value, field: &str) -> &'a str {
    body[field]
        .as_str()
        .unwrap_or_else(|| panic!("campo {field} ausente em {body}"))
}

#[tokio::test]
async fn fluxo_completo_cadastro_login_e_acesso_sob_rls() {
    let app = TestApp::spawn().await;

    let reply = signup(&app, "Ana@Exemplo.com", "senha-forte-123").await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    let session = reply.body;
    assert_eq!(session["token_type"], "bearer");
    assert_eq!(session["user"]["email"], "ana@exemplo.com");
    let user_id: Uuid = str_field(&session["user"], "id").parse().unwrap();

    // O JWT emitido é EdDSA e traz as claims do contrato.
    let access = str_field(&session, "access_token");
    let header = jsonwebtoken::decode_header(access).unwrap();
    assert_eq!(header.alg, Algorithm::EdDSA);
    assert!(header.kid.is_some());

    let (status, me) = app.get("/auth/v1/user", Some(access)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["id"], user_id.to_string());

    // O token do auth funciona na API sob RLS: Ana só vê a própria tarefa.
    app.admin_client
        .execute(
            "INSERT INTO public.todos (user_id, title) VALUES ($1, 'tarefa da Ana')",
            &[&user_id],
        )
        .await
        .unwrap();
    let (status, todos) = app.get("/rest/v1/todos", Some(access)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(todos, json!([todos[0].clone()]));
    assert_eq!(todos[0]["title"], "tarefa da Ana");

    // Login com a mesma senha (email em outra caixa) abre outra sessão.
    let reply = login(&app, "ANA@exemplo.com", "senha-forte-123").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_ne!(
        str_field(&reply.body, "refresh_token"),
        str_field(&session, "refresh_token")
    );
    assert!(reply.body["user"]["last_sign_in_at"].is_string());
}

#[tokio::test]
async fn senha_armazenada_em_phc_argon2id() {
    let app = TestApp::spawn().await;
    assert_eq!(
        signup(&app, "bia@exemplo.com", "senha-forte-123")
            .await
            .status,
        StatusCode::CREATED
    );
    let hash: String = app
        .admin_client
        .query_one(
            "SELECT encrypted_password FROM auth.users WHERE email = 'bia@exemplo.com'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(hash.starts_with("$argon2id$v=19$"), "{hash}");
    assert!(!hash.contains("senha-forte-123"));

    // Refresh tokens também não são guardados em claro.
    let stored: i64 = app
        .admin_client
        .query_one(
            "SELECT count(*) FROM auth.refresh_tokens WHERE length(token_hash) = 32",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored, 1);
}

#[tokio::test]
async fn cadastro_valida_entrada_e_email_duplicado() {
    let app = TestApp::spawn().await;
    assert_eq!(
        signup(&app, "caio@exemplo.com", "senha-forte-123")
            .await
            .status,
        StatusCode::CREATED
    );
    let dup = signup(&app, "CAIO@exemplo.com", "outra-senha-456").await;
    assert_eq!(dup.status, StatusCode::CONFLICT);
    assert_eq!(dup.body["code"], "user_already_exists");

    assert_eq!(
        signup(&app, "nao-e-email", "senha-forte-123").await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        signup(&app, "dani@exemplo.com", "curta").await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn login_invalido_nao_revela_se_email_existe() {
    let app = TestApp::spawn().await;
    signup(&app, "edu@exemplo.com", "senha-forte-123").await;

    let wrong = login(&app, "edu@exemplo.com", "senha-errada-000").await;
    let unknown = login(&app, "ninguem@exemplo.com", "senha-errada-000").await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);
    assert_eq!(wrong.body, unknown.body);
    assert_eq!(wrong.body["code"], "invalid_grant");
}

#[tokio::test]
async fn refresh_rotaciona_e_reuso_invalida_a_familia() {
    let app = TestApp::spawn().await;
    let session = signup(&app, "fabi@exemplo.com", "senha-forte-123")
        .await
        .body;
    let r1 = str_field(&session, "refresh_token").to_owned();

    // Uso legítimo: R1 -> R2.
    let rotated = refresh(&app, &r1).await;
    assert_eq!(rotated.status, StatusCode::OK, "{}", rotated.body);
    let r2 = str_field(&rotated.body, "refresh_token").to_owned();
    assert_ne!(r1, r2);
    assert!(rotated.body["access_token"].is_string());

    // R2 -> R3 continua funcionando.
    let rotated = refresh(&app, &r2).await;
    assert_eq!(rotated.status, StatusCode::OK);
    let r3 = str_field(&rotated.body, "refresh_token").to_owned();

    // Reuso de R1 (roubado): rejeitado e a sessão inteira é revogada...
    let reuse = refresh(&app, &r1).await;
    assert_eq!(reuse.status, StatusCode::BAD_REQUEST);
    assert_eq!(reuse.body["code"], "invalid_grant");

    // ...inclusive o token mais novo da família.
    let after = refresh(&app, &r3).await;
    assert_eq!(after.status, StatusCode::BAD_REQUEST);

    // Outras sessões do mesmo usuário não são afetadas.
    let other = login(&app, "fabi@exemplo.com", "senha-forte-123")
        .await
        .body;
    assert_eq!(
        refresh(&app, str_field(&other, "refresh_token"))
            .await
            .status,
        StatusCode::OK
    );

    assert_eq!(
        refresh(&app, "token-inexistente").await.status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn logout_encerra_a_sessao() {
    let app = TestApp::spawn().await;
    let session = signup(&app, "gabi@exemplo.com", "senha-forte-123")
        .await
        .body;
    let access = str_field(&session, "access_token");

    let reply = app.post("/auth/v1/logout", Some(access), json!({})).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(
        refresh(&app, str_field(&session, "refresh_token"))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );

    // Logout exige um JWT de usuário.
    let reply = app.post("/auth/v1/logout", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    let reply = app
        .post("/auth/v1/logout", Some(&service_token()), json!({}))
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn jwks_permite_validar_tokens_fora_do_nelcota() {
    let app = TestApp::spawn().await;
    let session = signup(&app, "hugo@exemplo.com", "senha-forte-123")
        .await
        .body;

    let reply = app
        .request(
            axum::http::Method::GET,
            "/auth/v1/.well-known/jwks.json",
            None,
            None,
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.headers.contains_key(header::CACHE_CONTROL));
    let jwks: JwkSet = serde_json::from_value(reply.body).unwrap();
    assert_eq!(jwks.keys.len(), 1);

    // Um serviço externo valida o token só com a chave pública.
    let key = DecodingKey::from_jwk(&jwks.keys[0]).unwrap();
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_audience(&["authenticated"]);
    validation.set_issuer(&["nelcota-test"]);
    let data =
        jsonwebtoken::decode::<Value>(str_field(&session, "access_token"), &key, &validation)
            .unwrap();
    assert_eq!(data.claims["role"], "authenticated");
    assert_eq!(data.claims["email"], "hugo@exemplo.com");
    assert!(data.claims["session_id"].is_string());
}

#[tokio::test]
async fn rate_limit_em_login() {
    let app = TestApp::spawn_with(Options {
        rate_limit_per_minute: 3,
        ..Options::default()
    })
    .await;
    for _ in 0..3 {
        let reply = login(&app, "ivo@exemplo.com", "senha-errada-000").await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    }
    let blocked = login(&app, "ivo@exemplo.com", "senha-errada-000").await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(blocked.headers.contains_key(header::RETRY_AFTER));
}

/// As tabelas `auth.*` não são acessíveis pelas roles da API.
#[tokio::test]
async fn roles_da_api_nao_leem_o_schema_auth() {
    let app = TestApp::spawn().await;
    signup(&app, "julia@exemplo.com", "senha-forte-123").await;

    for payload in [
        json!({ "role": "anon" }),
        json!({ "role": "authenticated", "sub": app.user_a }),
        json!({ "role": "service_role" }),
    ] {
        let claims = Claims::from_payload(payload.clone()).unwrap();
        for query in [
            "SELECT encrypted_password FROM auth.users",
            "SELECT token_hash FROM auth.one_time_tokens",
        ] {
            // Uma transação por consulta: o erro aborta a transação inteira.
            let mut client = app.pool.get().await.unwrap();
            let tx = db::begin_request(&mut client, &claims).await.unwrap();
            let err = tx.query(query, &[]).await.unwrap_err();
            assert_eq!(
                err.code(),
                Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE),
                "{payload}: {query}"
            );
        }
    }

    // E nenhum JWT consegue assumir a role interna do auth.
    let (status, _) = app
        .get(
            "/rest/v1/todos",
            Some(&token(json!({ "role": "nelcota_auth" }))),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------- recuperação de senha

async fn recover(app: &TestApp, email: &str) -> Reply {
    app.post("/auth/v1/recover", None, json!({ "email": email }))
        .await
}

async fn verify(app: &TestApp, token: &str, password: &str) -> Reply {
    app.post(
        "/auth/v1/verify",
        None,
        json!({ "type": "recovery", "token": token, "password": password }),
    )
    .await
}

/// Token do link do email (`...#type=recovery&token=<token>`).
fn link_token(email: &nelcota_auth::Email) -> String {
    let link = email
        .text
        .split_whitespace()
        .find(|word| word.starts_with(RECOVERY_URL))
        .unwrap_or_else(|| panic!("sem link em: {}", email.text));
    link.strip_prefix(&format!("{RECOVERY_URL}#type=recovery&token="))
        .unwrap_or_else(|| panic!("link fora do formato: {link}"))
        .to_owned()
}

#[tokio::test]
async fn recuperacao_de_senha_troca_a_senha_e_encerra_as_sessoes() {
    let app = TestApp::spawn().await;
    let old = signup(&app, "ana@exemplo.com", "senha-antiga-123")
        .await
        .body;
    let old_refresh = str_field(&old, "refresh_token").to_owned();
    assert!(old["user"]["email_confirmed_at"].is_null());

    let reply = recover(&app, "Ana@Exemplo.com").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body, json!({}));
    let sent = app.outbox.wait_for(1).await;
    assert_eq!(sent[0].to, "ana@exemplo.com");
    let token = link_token(&sent[0]);

    // Só o SHA-256 do token fica no banco.
    let row = app
        .admin_client
        .query_one(
            "SELECT count(*) FILTER (WHERE token_hash = sha256($1::text::bytea)),
                    count(*) FILTER (WHERE position($1::text::bytea IN token_hash) > 0)
             FROM auth.one_time_tokens",
            &[&token],
        )
        .await
        .unwrap();
    assert_eq!((row.get::<_, i64>(0), row.get::<_, i64>(1)), (1, 0));

    let reply = verify(&app, &token, "senha-nova-456").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["user"]["email"], "ana@exemplo.com");
    // Abrir o link prova que a pessoa recebe os emails da conta.
    assert!(reply.body["user"]["email_confirmed_at"].is_string());
    assert!(reply.body["access_token"].is_string());

    assert_eq!(
        login(&app, "ana@exemplo.com", "senha-antiga-123")
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        login(&app, "ana@exemplo.com", "senha-nova-456")
            .await
            .status,
        StatusCode::OK
    );
    // A sessão aberta com a senha antiga foi encerrada.
    let reply = refresh(&app, &old_refresh).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body["code"], "invalid_grant");

    // O link vale uma vez só.
    let reply = verify(&app, &token, "outra-senha-789").await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body["code"], "invalid_grant");
}

#[tokio::test]
async fn recuperacao_nao_revela_se_a_conta_existe() {
    let app = TestApp::spawn().await;
    signup(&app, "bia@exemplo.com", "senha-forte-123").await;

    let existing = recover(&app, "bia@exemplo.com").await;
    let missing = recover(&app, "ninguem@exemplo.com").await;
    assert_eq!(
        (existing.status, &existing.body),
        (missing.status, &missing.body)
    );
    app.outbox.wait_for(1).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(app.outbox.sent().len(), 1, "só a conta que existe recebe");

    let reply = recover(&app, "não é email").await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn recuperacao_manda_um_email_por_minuto_e_so_o_ultimo_link_vale() {
    let app = TestApp::spawn().await;
    signup(&app, "caio@exemplo.com", "senha-forte-123").await;

    recover(&app, "caio@exemplo.com").await;
    let first = link_token(&app.outbox.wait_for(1).await[0]);
    // Pedido repetido logo em seguida: mesma resposta, nenhum email novo.
    assert_eq!(
        recover(&app, "caio@exemplo.com").await.status,
        StatusCode::OK
    );
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(app.outbox.sent().len(), 1);

    // Passado o intervalo, um novo pedido manda outro link e invalida o anterior.
    app.admin_client
        .execute(
            "UPDATE auth.one_time_tokens SET created_at = now() - interval '2 minutes'",
            &[],
        )
        .await
        .unwrap();
    recover(&app, "caio@exemplo.com").await;
    let second = link_token(&app.outbox.wait_for(2).await[1]);
    assert_ne!(first, second);
    assert_eq!(
        verify(&app, &first, "senha-nova-456").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        verify(&app, &second, "senha-nova-456").await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn link_expirado_invalido_ou_senha_fraca() {
    let app = TestApp::spawn().await;
    signup(&app, "davi@exemplo.com", "senha-forte-123").await;
    recover(&app, "davi@exemplo.com").await;
    let token = link_token(&app.outbox.wait_for(1).await[0]);

    // Senha fora das regras não consome o link.
    let reply = verify(&app, &token, "curta").await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);

    for bad in ["", "token-inventado", &"x".repeat(200)] {
        let reply = verify(&app, bad, "senha-nova-456").await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let reply = app
        .post(
            "/auth/v1/verify",
            None,
            json!({ "type": "signup", "token": token, "password": "senha-nova-456" }),
        )
        .await;
    assert_eq!(reply.body["code"], "unsupported_type");

    app.admin_client
        .execute(
            "UPDATE auth.one_time_tokens SET expires_at = now() - interval '1 second'",
            &[],
        )
        .await
        .unwrap();
    let reply = verify(&app, &token, "senha-nova-456").await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        login(&app, "davi@exemplo.com", "senha-forte-123")
            .await
            .status,
        StatusCode::OK,
        "a senha antiga continua valendo"
    );
}

#[tokio::test]
async fn recuperacao_desligada_sem_smtp() {
    let app = TestApp::spawn_with(Options {
        mail: false,
        ..Options::default()
    })
    .await;
    signup(&app, "eva@exemplo.com", "senha-forte-123").await;
    let reply = recover(&app, "eva@exemplo.com").await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.body["code"], "recovery_disabled");
}
