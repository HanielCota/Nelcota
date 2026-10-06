# Registro de decisões

Decisões tomadas durante a construção, com o porquê. Mais recentes no fim.

## Marco 1

**D1. Licença Apache-2.0.** Escolhida pelo mantenedor (2026-10-06).

**D2. Nome e binário.** O projeto se chama Nelcota; o binário único é `nelcota`
(crate `nelcota-server`). No Marco 4 ele ganha subcomandos (`nelcota init`,
`nelcota up`...); hoje, sem argumentos, ele sobe o servidor.

**D3. Crates `admin` e `cli` só entram nos Marcos 4/5.** Evita crates vazias.

**D4. Testes de integração em `crates/server/tests/`.** O workspace é virtual
(não há pacote na raiz), e testes de integração no Cargo precisam pertencer a um
pacote. O `server` expõe o `Router` numa lib para os testes usarem o mesmo código
de produção.

**D5. Uma URL só.** `NELCOTA_DATABASE_URL` aponta para uma role dona do schema
(o superusuário `postgres` no Docker) e é usada apenas para migrações e para
definir a senha do `authenticator`. A API usa o mesmo host/banco, trocando
usuário/senha por `authenticator` / `NELCOTA_AUTHENTICATOR_PASSWORD`.

**D6. Senha do `authenticator` fora das migrações.** Migrações não contêm
segredos. Na inicialização o servidor calcula o verificador SCRAM-SHA-256 no
cliente e executa `ALTER ROLE ... PASSWORD 'SCRAM-SHA-256$...'`: a senha em
texto puro nunca trafega nem aparece no log de statements do Postgres.

**D7. `set_config('role', $1, true)` em vez de `SET LOCAL ROLE x`.** Os dois são
equivalentes (`SET LOCAL ROLE` é açúcar para `set_config('role', ..., true)`), mas
`set_config` aceita parâmetro, junta role e claims numa única ida ao banco e evita
qualquer SQL montado como string. O valor da role vem de um `enum` fechado.

**D8. Pool com `RecyclingMethod::Fast`.** Role e claims têm escopo de transação e
morrem no COMMIT/ROLLBACK (inclusive no drop sem commit, que manda ROLLBACK).
Não precisamos de `DISCARD ALL` a cada checkout. Testado em
`role_e_claims_nao_vazam_entre_requests_do_pool` e
`transacao_com_erro_faz_rollback_da_role`.

**D9. Migrações internas em `nelcota.schema_migrations`.** Schema próprio, sem
GRANT para as roles da API, para não aparecer no `public` (que será exposto pela
API automática). Um advisory lock serializa o bootstrap entre instâncias.

**D10. Tabela de exemplo fora das migrações.** `examples/todos.sql` é aplicado
pelos testes e opcionalmente em dev. As migrações do Nelcota não criam tabelas
de negócio no banco de ninguém.

**D11. Extensões no schema `extensions`.** `pgcrypto` e `pg_stat_statements`
ficam fora do `public`. `pg_stat_statements` exige
`shared_preload_libraries=pg_stat_statements` (já no compose de dev e nos testes).

**D12. JWT HS256 provisório.** O Marco 1 usa HS256 com segredo compartilhado
(≥ 32 caracteres) atrás da trait `JwtVerifier`. O Marco 2 adiciona EdDSA + JWKS
como padrão, e HS256 fica como modo simples.

**D13. Regras das claims.** `role` é obrigatória e precisa ser `anon`,
`authenticated` ou `service_role`. `authenticated` exige `sub` em formato uuid
(para `auth.uid()` nunca falhar no cast). `aud` ainda não é verificada (entra no
contrato no Marco 2). Leeway de 30 s em `exp`.

**D14. Sem token = `anon`; token inválido = 401.** Um `Authorization` presente
mas inválido (expirado, assinatura errada, esquema diferente de `Bearer`, role
desconhecida) nunca vira `anon` silenciosamente.

**D15. 401 vs 403 em falta de permissão.** `insufficient_privilege` (42501) vira
401 para `anon` (precisa logar) e 403 para as demais roles, como no PostgREST.

**D16. `rsa` ignorada no `cargo audit`/`cargo deny` (RUSTSEC-2023-0071).** Vem
pelo backend `rust_crypto` do `jsonwebtoken`. O Marvin Attack afeta operações
com a chave **privada** RSA; o Nelcota nunca assina com RSA. O backend
`aws_lc_rs` evitaria a dependência, mas exige toolchain C/CMake no build, o que
fere o "binário estático simples". Revisar se RS256 for usado para assinar.

**D17. Libs fora da tabela do prompt.**
- `uuid`: valida a claim `sub` e é o tipo nativo do Postgres para ids.
- `postgres-protocol`: já é dependência transitiva do `tokio-postgres`; usada
  para calcular o verificador SCRAM (D6).
- `http-body-util` (só dev): ler o corpo das respostas nos testes.

**D18. Postgres sem TLS na rede interna.** O banco só é acessível pela rede
interna do Docker (ou localhost em dev). TLS para o banco fica para quando houver
um cenário de banco remoto.

**D19. Edition 2024, MSRV 1.88.** O `testcontainers-modules` atual já exige 1.88.

### Pendências conhecidas

- `CompressionLayer` (tower-http) entra junto com a API automática no Marco 3.
- `statement_timeout` por role (ex.: `ALTER ROLE anon SET statement_timeout`)
  a definir no Marco 3, junto com os testes de desempenho.
