# Arquitetura

```
cliente ──HTTPS──▶ Caddy ──▶ nelcota (binário único) ──▶ Postgres 17
                              │ axum: /health, /rest/v1/*
                              │ auth: valida JWT → Claims
                              │ core: pool (authenticator) + transação com role/claims
```

## Crates

| Crate | Responsabilidade |
|---|---|
| `nelcota-core` | config (`figment`), erros HTTP, `Claims`/`Role`, pool, migrações, `begin_request` |
| `nelcota-auth` | trait `JwtVerifier`, verificador HS256, extrator `Auth` do axum |
| `nelcota-api`  | rotas REST (Marco 1: só `GET /rest/v1/todos`; Marco 3: CRUD automático) |
| `nelcota-server` | binário `nelcota`: junta tudo, middlewares (trace, timeout, CORS) |

Planejados: `nelcota-cli` (Marco 4) e `nelcota-admin` (Marco 5), compilados no
mesmo binário.

## Fluxo de um request

1. `Auth` (extrator) transforma o header `Authorization` em `Claims`.
2. O handler pega uma conexão do pool (sempre como `authenticator`).
3. `db::begin_request` abre a transação e define role + claims.
4. A query roda sob RLS; o Postgres monta o JSON (`json_agg`), e a API só
   repassa os bytes.
5. COMMIT. Erros do Postgres são traduzidos por `ApiError::from_db`.

`db::begin_request` é o **único** caminho para executar SQL em nome de um
usuário.

## Inicialização

1. Carrega a config (`NELCOTA_*` + `nelcota.toml` opcional).
2. `db::bootstrap` (conexão administrativa): advisory lock → migrações em
   `migrations/` (tabela de controle `nelcota.schema_migrations`) → senha do
   `authenticator` (verificador SCRAM calculado localmente).
3. Cria o pool da API e sobe o servidor HTTP. Encerra de forma limpa com
   SIGTERM/Ctrl+C.

## Migrações internas

SQL puro em `migrations/V<n>__<nome>.sql`, aplicadas pelo `refinery` e
registradas em `nelcota.schema_migrations` (versão, nome, data, checksum). Podem
ser aplicadas à mão com `psql` se o Nelcota não existir mais.
