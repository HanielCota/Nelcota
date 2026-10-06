# Nelcota

BaaS open source, simples e sem lock-in: **Postgres puro + um binário Rust +
deploy em 5 minutos numa VPS.**

- **O Postgres é o produto.** Schema, policies RLS e funções são SQL puro e
  continuam funcionando sem o Nelcota.
- **A autorização é do RLS.** A API só valida o JWT e assume a role dele
  dentro de uma transação.
- **Padrões abertos.** JWT, argon2, SQL puro, S3-compatible.

> Status: **Marco 1 (núcleo)**. Fluxo JWT → role → RLS funcionando e testado.
> Auth completo, API automática, CLI e painel vêm nos próximos marcos.

## Quickstart (desenvolvimento)

Requisitos: Rust stable, Docker.

```sh
cp .env.example .env              # troque os segredos
docker compose up -d --wait       # Postgres 17 em 127.0.0.1:5432

set -a; . ./.env; set +a          # carrega as variáveis NELCOTA_*
cargo run                         # aplica as migrações e sobe em :8000

curl localhost:8000/health        # {"status":"ok"}
```

Tabela de exemplo com RLS (opcional):

```sh
docker compose exec -T postgres psql -U postgres < examples/todos.sql
curl -i localhost:8000/rest/v1/todos   # 401: anon não acessa
```

## Testes

```sh
cargo test                              # requer Docker (testcontainers sobe Postgres 17)
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
cargo audit && cargo deny check
```

Os testes de integração (`crates/server/tests/rls.rs`) provam, contra um
Postgres real, que um usuário não lê dados de outro, que `anon` não acessa
tabelas protegidas, que JWTs inválidos/expirados/com role desconhecida dão 401
e que role/claims não vazam entre requests do pool.

## Documentação

- [Arquitetura](docs/arquitetura.md)
- [JWT e roles](docs/jwt-e-roles.md)
- [Registro de decisões](docs/decisoes.md)

> ⚠️ Um JWT com `role: service_role` ignora todo o RLS. Nunca o exponha no
> frontend.

## Licença

[Apache-2.0](LICENSE)
