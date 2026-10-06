//! Testes da API REST automática contra Postgres real: CRUD, filtros, RLS em
//! todos os verbos, injeção de SQL, RPC, OpenAPI, recarga do catálogo e um
//! teste básico de desempenho.

mod common;

use std::time::{Duration, Instant};

use axum::http::{Method, StatusCode, header};
use common::*;
use serde_json::{Value, json};

const REPR: (&str, &str) = ("prefer", "return=representation");

fn ids(body: &Value) -> Vec<i64> {
    body.as_array()
        .unwrap_or_else(|| panic!("esperava array: {body}"))
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect()
}

fn names(body: &Value) -> Vec<&str> {
    body.as_array()
        .unwrap_or_else(|| panic!("esperava array: {body}"))
        .iter()
        .map(|r| r["nome"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn crud_completo_com_representation() {
    let app = TestApp::spawn().await;
    let a = user_token(app.user_a);

    // POST com representação: user_id vem do DEFAULT auth.uid().
    let created = app
        .request_with(
            Method::POST,
            "/rest/v1/todos",
            Some(&a),
            Some(json!({ "title": "nova", "tags": ["casa", "urgente"], "extra": { "x": 1 } })),
            &[REPR],
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let row = &created.body[0];
    assert_eq!(row["user_id"], app.user_a.to_string());
    assert_eq!(row["prioridade"], "media");
    assert_eq!(row["tags"], json!(["casa", "urgente"]));
    assert_eq!(row["extra"], json!({ "x": 1 }));
    let id = row["id"].as_i64().unwrap();

    // POST mínimo: 201 sem corpo.
    let minimal = app
        .post(
            "/rest/v1/todos",
            Some(&a),
            json!([{ "title": "x" }, { "title": "y" }]),
        )
        .await;
    assert_eq!(minimal.status, StatusCode::CREATED);
    assert_eq!(minimal.body, Value::Null);

    let (status, body) = app
        .get(
            &format!("/rest/v1/todos?id=eq.{id}&select=id,title,done"),
            Some(&a),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([{ "id": id, "title": "nova", "done": false }]));

    let patched = app
        .request_with(
            Method::PATCH,
            &format!("/rest/v1/todos?id=eq.{id}&select=id,done"),
            Some(&a),
            Some(json!({ "done": true, "prioridade": "alta" })),
            &[REPR],
        )
        .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    assert_eq!(patched.body, json!([{ "id": id, "done": true }]));

    let minimal = app
        .request(
            Method::PATCH,
            &format!("/rest/v1/todos?id=eq.{id}"),
            Some(&a),
            Some(json!({ "title": "renomeada" })),
        )
        .await;
    assert_eq!(minimal.status, StatusCode::NO_CONTENT);

    let deleted = app
        .request_with(
            Method::DELETE,
            &format!("/rest/v1/todos?id=eq.{id}"),
            Some(&a),
            None,
            &[REPR],
        )
        .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert_eq!(deleted.body[0]["title"], "renomeada");

    let (_, body) = app
        .get(&format!("/rest/v1/todos?id=eq.{id}"), Some(&a))
        .await;
    assert_eq!(body, json!([]));
}

#[tokio::test]
async fn rls_vale_em_todos_os_verbos() {
    let app = TestApp::spawn().await;
    let a = user_token(app.user_a);
    let b_id: i64 = app
        .admin_client
        .query_one(
            "SELECT id FROM public.todos WHERE user_id = $1",
            &[&app.user_b],
        )
        .await
        .unwrap()
        .get(0);

    // GET: A não vê a linha de B nem filtrando pelo id dela.
    let (_, body) = app
        .get(&format!("/rest/v1/todos?id=eq.{b_id}"), Some(&a))
        .await;
    assert_eq!(body, json!([]));

    // PATCH/DELETE na linha de B: zero linhas afetadas.
    for method in [Method::PATCH, Method::DELETE] {
        let reply = app
            .request_with(
                method.clone(),
                &format!("/rest/v1/todos?id=eq.{b_id}"),
                Some(&a),
                (method == Method::PATCH).then(|| json!({ "title": "invadido" })),
                &[REPR],
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{method}");
        assert_eq!(reply.body, json!([]), "{method}");
    }

    // POST em nome de B: violação da policy (WITH CHECK).
    let reply = app
        .post(
            "/rest/v1/todos",
            Some(&a),
            json!({ "title": "forjada", "user_id": app.user_b }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN, "{}", reply.body);

    // PATCH tentando transferir a própria linha para B: também barrado.
    let reply = app
        .request(
            Method::PATCH,
            "/rest/v1/todos?title=eq.tarefa%20de%20A",
            Some(&a),
            Some(json!({ "user_id": app.user_b })),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);

    // A linha de B continua intacta.
    let title: String = app
        .admin_client
        .query_one("SELECT title FROM public.todos WHERE id = $1", &[&b_id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(title, "tarefa de B");

    // anon: sem GRANT, 401 em todos os verbos.
    for (method, body) in [
        (Method::GET, None),
        (Method::POST, Some(json!({ "title": "x" }))),
        (Method::PATCH, Some(json!({ "title": "x" }))),
        (Method::DELETE, None),
    ] {
        let path = if method == Method::POST {
            "/rest/v1/todos"
        } else {
            "/rest/v1/todos?id=gt.0"
        };
        let reply = app.request(method.clone(), path, None, body).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{method}");
    }

    // authenticated sem GRANT de escrita em produtos: 403. service_role: ok.
    let reply = app
        .post(
            "/rest/v1/produtos",
            Some(&a),
            json!({ "nome": "X", "preco": 1 }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    let reply = app
        .post(
            "/rest/v1/produtos",
            Some(&service_token()),
            json!({ "nome": "X", "preco": 1 }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED);

    // Tabela sem GRANT para usuários.
    let (status, _) = app.get("/rest/v1/segredos", Some(&a)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = app.get("/rest/v1/segredos", Some(&service_token())).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn filtros_ordem_paginacao_e_contagem() {
    let app = TestApp::spawn().await;
    let get = |q: &str| {
        let path = format!("/rest/v1/produtos?{q}");
        let app = &app;
        async move { app.get(&path, None).await }
    };

    let (_, b) = get("preco=gt.10&order=preco.desc").await;
    assert_eq!(names(&b), ["Mochila", "Caderno"]);
    let (_, b) = get("estoque=eq.0").await;
    assert_eq!(names(&b), ["Mochila"]);
    let (_, b) = get("estoque=neq.0&order=nome").await;
    assert_eq!(names(&b), ["Caderno", "Caneta", "Régua, 30cm"]);
    let (_, b) = get("preco=lte.4&preco=gte.3").await;
    assert_eq!(names(&b), ["Régua, 30cm"]);
    let (_, b) = get("nome=in.(Caneta,\"Régua, 30cm\")&order=id").await;
    assert_eq!(names(&b), ["Caneta", "Régua, 30cm"]);
    let (_, b) = get("nome=in.()").await;
    assert_eq!(b, json!([]));
    let (_, b) = get("nome=like.Ca*&order=nome").await;
    assert_eq!(names(&b), ["Caderno", "Caneta"]);
    let (_, b) = get("nome=ilike.*CHILA").await;
    assert_eq!(names(&b), ["Mochila"]);
    let (_, b) = get("nome=not.like.Ca*&order=nome.desc").await;
    assert_eq!(names(&b), ["Régua, 30cm", "Mochila"]);
    let (_, b) = get("slug=eq.mochila&select=nome,slug").await;
    assert_eq!(b, json!([{ "nome": "Mochila", "slug": "mochila" }]));
    let (_, b) = get("criado_em=not.is.null&select=id&order=id&limit=2&offset=1").await;
    assert_eq!(ids(&b), [2, 3]);

    // Content-Range e count=exact.
    let reply = app
        .request_with(
            Method::GET,
            "/rest/v1/produtos?order=id&limit=2&offset=1",
            None,
            None,
            &[("prefer", "count=exact")],
        )
        .await;
    assert_eq!(reply.headers[header::CONTENT_RANGE], "1-2/4");
    let reply = app
        .request(Method::GET, "/rest/v1/produtos?estoque=gt.1000", None, None)
        .await;
    assert_eq!(reply.headers[header::CONTENT_RANGE], "*/*");

    // Filtros em enum, array e booleano (como usuário).
    let a = user_token(app.user_a);
    // Lote com chaves diferentes: colunas ausentes recebem o DEFAULT.
    let reply = app
        .post(
            "/rest/v1/todos",
            Some(&a),
            json!([
                { "title": "alta", "prioridade": "alta", "tags": ["x"] },
                { "title": "baixa", "prioridade": "baixa", "done": true },
            ]),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    let (_, b) = app
        .get("/rest/v1/todos?prioridade=eq.alta&select=title", Some(&a))
        .await;
    assert_eq!(b, json!([{ "title": "alta" }]));
    let (_, b) = app
        .get("/rest/v1/todos?done=is.true&select=title", Some(&a))
        .await;
    assert_eq!(b, json!([{ "title": "baixa" }]));
    let (_, b) = app
        .get(
            "/rest/v1/todos?prioridade=in.(alta,baixa)&select=title&order=title",
            Some(&a),
        )
        .await;
    assert_eq!(b, json!([{ "title": "alta" }, { "title": "baixa" }]));
}

#[tokio::test]
async fn max_rows_limita_leituras() {
    let app = TestApp::spawn_with(Options {
        max_rows: Some(2),
        ..Options::default()
    })
    .await;
    let (_, b) = app.get("/rest/v1/produtos?order=id", None).await;
    assert_eq!(ids(&b), [1, 2]);
    let (_, b) = app.get("/rest/v1/produtos?order=id&limit=100", None).await;
    assert_eq!(ids(&b).len(), 2);
}

#[tokio::test]
async fn erros_de_dados_viram_400() {
    let app = TestApp::spawn().await;
    let s = service_token();
    for (case, body) in [
        ("check", json!({ "nome": "X", "preco": -1 })),
        ("not null", json!({ "preco": 1 })),
        ("tipo", json!({ "nome": "X", "preco": "caro" })),
        (
            "coluna gerada",
            json!({ "nome": "X", "preco": 1, "slug": "y" }),
        ),
        (
            "coluna inexistente",
            json!({ "nome": "X", "preco": 1, "cor": "azul" }),
        ),
    ] {
        let reply = app.post("/rest/v1/produtos", Some(&s), body).await;
        assert_eq!(
            reply.status,
            StatusCode::BAD_REQUEST,
            "{case}: {}",
            reply.body
        );
    }
    let (status, _) = app.get("/rest/v1/produtos?preco=eq.abc", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = app
        .get("/rest/v1/produtos?id=gt.1&estoque=lt.x", None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Duplicata de PK: 409.
    let reply = app
        .post(
            "/rest/v1/produtos",
            Some(&s),
            json!({ "id": 1, "nome": "X", "preco": 1 }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);

    // Corpo que não é JSON.
    let reply = app
        .request_with(Method::POST, "/rest/v1/produtos", Some(&s), None, &[])
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);

    // PATCH/DELETE sem filtro são recusados.
    let reply = app
        .request(Method::DELETE, "/rest/v1/produtos", Some(&s), None)
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    let reply = app
        .request(
            Method::PATCH,
            "/rest/v1/produtos",
            Some(&s),
            Some(json!({ "estoque": 0 })),
        )
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);

    let (status, _) = app.get("/rest/v1/nao_existe", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn injecao_de_sql_em_identificadores_e_valores() {
    let app = TestApp::spawn().await;
    let s = service_token();

    // Identificadores maliciosos: rejeitados antes de chegar ao banco.
    for path in [
        "/rest/v1/produtos;DROP%20TABLE%20produtos",
        "/rest/v1/produtos%22;DROP%20TABLE%20produtos;--",
        "/rest/v1/pg_catalog.pg_authid",
        "/rest/v1/..%2Fauth.users",
    ] {
        let (status, _) = app.get(path, Some(&s)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    for query in [
        "select=nome,(SELECT%20senha%20FROM%20auth.users)",
        "select=nome%22,%22preco",
        "select=*;DROP%20TABLE%20produtos",
        "order=nome;DROP%20TABLE%20produtos",
        "order=(SELECT%201)",
        "nome%22%3D%27x%27%20OR%201%3D1--=eq.1",
        "id=eq.1&1=1",
        "limit=1;DROP%20TABLE%20produtos",
        "id=in.(1)%29%20OR%20(1=1",
    ] {
        let (status, body) = app
            .get(&format!("/rest/v1/produtos?{query}"), Some(&s))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {body}");
    }

    // Valores maliciosos: tratados como dado.
    let payload = "x'); DROP TABLE produtos; --";
    let (status, body) = app
        .get(
            &format!("/rest/v1/produtos?nome=eq.{}", urlencode(payload)),
            Some(&s),
        )
        .await;
    assert_eq!((status, body), (StatusCode::OK, json!([])));
    let (status, _) = app
        .get(
            &format!(
                "/rest/v1/produtos?nome=in.({},\"a\\\"b\")",
                urlencode(payload)
            ),
            Some(&s),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let created = app
        .request_with(
            Method::POST,
            "/rest/v1/produtos",
            Some(&s),
            Some(json!({ "nome": payload, "preco": 1 })),
            &[REPR],
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    assert_eq!(created.body[0]["nome"], payload);

    // Chaves maliciosas no corpo e nomes de função.
    let reply = app
        .post(
            "/rest/v1/produtos",
            Some(&s),
            json!({ "nome\" text); DROP TABLE produtos; --": "x" }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    let reply = app
        .post(
            "/rest/v1/rpc/soma;DROP%20TABLE%20produtos",
            Some(&s),
            json!({}),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    let reply = app
        .post(
            "/rest/v1/rpc/soma",
            Some(&s),
            json!({ "a) ; DROP TABLE produtos; --": 1 }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);

    // Tudo continua lá.
    let count: i64 = app
        .admin_client
        .query_one("SELECT count(*) FROM public.produtos", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 5);
}

fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[tokio::test]
async fn rpc_chama_funcoes_sob_a_role_do_jwt() {
    let app = TestApp::spawn().await;
    let a = user_token(app.user_a);

    let reply = app.post("/rest/v1/rpc/soma", None, json!({ "a": 1 })).await;
    assert_eq!((reply.status, reply.body), (StatusCode::OK, json!(11)));
    let reply = app
        .post("/rest/v1/rpc/soma", None, json!({ "a": 1, "b": 2 }))
        .await;
    assert_eq!(reply.body, json!(3));
    let reply = app
        .post("/rest/v1/rpc/soma", None, json!({ "a": 1, "c": 2 }))
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    let reply = app.post("/rest/v1/rpc/soma", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);

    // Funções SETOF obedecem ao RLS da tabela.
    let reply = app
        .post("/rest/v1/rpc/minhas_tarefas_abertas", Some(&a), json!({}))
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body.as_array().unwrap().len(), 1);
    assert_eq!(reply.body[0]["title"], "tarefa de A");

    // auth.uid()/auth.role() dentro da função refletem o JWT.
    let reply = app
        .post("/rest/v1/rpc/quem_sou_eu", Some(&a), json!({}))
        .await;
    assert_eq!(
        reply.body,
        json!({ "uid": app.user_a, "role": "authenticated" })
    );
    let reply = app
        .request(Method::POST, "/rest/v1/rpc/quem_sou_eu", None, None)
        .await;
    assert_eq!(reply.body, json!({ "uid": null, "role": "anon" }));

    let reply = app.post("/rest/v1/rpc/nada", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let reply = app.post("/rest/v1/rpc/falha", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert!(
        reply.body["message"]
            .as_str()
            .unwrap()
            .contains("regra de negócio")
    );
    let reply = app.post("/rest/v1/rpc/nao_existe", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);

    // EXECUTE revogado de PUBLIC: anon 401, authenticated 403, service ok.
    let reply = app.post("/rest/v1/rpc/so_servico", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    let reply = app
        .post("/rest/v1/rpc/so_servico", Some(&a), json!({}))
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    let reply = app
        .post("/rest/v1/rpc/so_servico", Some(&service_token()), json!({}))
        .await;
    assert_eq!(reply.body, json!("ok"));
}

#[tokio::test]
async fn openapi_segue_os_privilegios_da_role() {
    let app = TestApp::spawn().await;

    let (status, anon) = app.get("/rest/v1/", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(anon["openapi"], "3.0.3");
    let paths = anon["paths"].as_object().unwrap();
    assert!(paths.contains_key("/produtos"));
    assert!(paths["/produtos"].get("get").is_some());
    assert!(paths["/produtos"].get("post").is_none());
    assert!(!paths.contains_key("/todos"));
    assert!(!paths.contains_key("/segredos"));
    assert!(paths.contains_key("/rpc/soma"));
    assert!(!paths.contains_key("/rpc/so_servico"));
    let produto = &anon["components"]["schemas"]["produtos"];
    assert_eq!(produto["properties"]["preco"]["type"], "number");
    assert_eq!(produto["properties"]["id"]["type"], "integer");
    assert_eq!(produto["description"], "Catálogo de produtos");

    let (_, user) = app.get("/rest/v1/", Some(&user_token(app.user_a))).await;
    let todos = &user["paths"]["/todos"];
    for verb in ["get", "post", "patch", "delete"] {
        assert!(todos.get(verb).is_some(), "{verb}");
    }
    let prioridade = &user["components"]["schemas"]["todos"]["properties"]["prioridade"];
    assert_eq!(prioridade["enum"], json!(["baixa", "media", "alta"]));

    let (_, service) = app.get("/rest/v1/", Some(&service_token())).await;
    assert!(service["paths"].get("/segredos").is_some());
    assert!(service["paths"].get("/rpc/so_servico").is_some());
}

#[tokio::test]
async fn catalogo_recarrega_sozinho_apos_ddl() {
    let app = TestApp::spawn().await;
    let (status, _) = app.get("/rest/v1/novidades", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    app.admin_client
        .batch_execute(
            "CREATE TABLE public.novidades (id int PRIMARY KEY, texto text);
             INSERT INTO public.novidades VALUES (1, 'olá');
             GRANT SELECT ON public.novidades TO anon;",
        )
        .await
        .unwrap();

    // O event trigger notifica; o listener recarrega em ~100 ms.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (status, body) = app.get("/rest/v1/novidades", None).await;
        if status == StatusCode::OK {
            assert_eq!(body, json!([{ "id": 1, "texto": "olá" }]));
            break;
        }
        assert!(Instant::now() < deadline, "catálogo não recarregou");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Colunas novas também aparecem.
    app.admin_client
        .batch_execute("ALTER TABLE public.novidades ADD COLUMN lida boolean DEFAULT false")
        .await
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while app
        .catalog
        .get()
        .table("novidades")
        .unwrap()
        .column("lida")
        .is_none()
    {
        assert!(Instant::now() < deadline, "coluna nova não apareceu");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (status, _) = app.get("/rest/v1/novidades?lida=is.false", None).await;
    assert_eq!(status, StatusCode::OK);
}

/// Não é benchmark (esse fica em `bench/`): só garante que a leitura simples
/// não degrada absurdamente, mesmo em build de debug.
#[tokio::test]
async fn desempenho_basico_de_leitura() {
    let app = TestApp::spawn().await;
    app.admin_client
        .batch_execute(
            "INSERT INTO public.produtos (nome, preco, estoque)
             SELECT 'produto ' || i, (i % 500) + 0.99, i % 7 FROM generate_series(1, 20000) i;
             ANALYZE public.produtos;",
        )
        .await
        .unwrap();

    let requests = 300;
    let started = Instant::now();
    for i in 0..requests {
        let (status, body) = app
            .get(
                &format!("/rest/v1/produtos?estoque=eq.{}&order=id&limit=20", i % 7),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 20);
    }
    let average = started.elapsed() / requests;
    println!("leitura simples: média de {average:?} por request ({requests} requests)");
    assert!(average < Duration::from_millis(100), "média {average:?}");
}
