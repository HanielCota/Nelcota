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

**D12. JWT HS256 provisório** (substituída por D20). O Marco 1 usa HS256 com segredo compartilhado
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

## Marco 2

**D20. EdDSA (Ed25519) como padrão; HS256 como legado.** A chave privada vem
em `NELCOTA_JWT_PRIVATE_KEY` (PEM PKCS#8 ou o base64 de uma linha, que cabe num
`.env`). Com chave EdDSA **e** segredo HS256 configurados, o servidor assina com
EdDSA e aceita os dois na verificação (migração sem downtime). O `kid` é o
thumbprint RFC 7638.

**D21. Role interna `nelcota_auth` para o schema `auth`.** Os handlers de auth
rodam com `SET LOCAL ROLE nelcota_auth` (string fixa). Nenhum JWT pode assumi-la
(o enum `Role` não a contém), e as roles da API não têm GRANT em `auth.*`.
Alternativa descartada: funções `SECURITY DEFINER`, que espalhariam lógica em
PL/pgSQL sem ganho real.

**D22. Refresh token opaco de 32 bytes, guardado só como SHA-256.** SHA-256 (e
não argon2) porque o token já tem 256 bits de entropia; a busca é por índice
único, sem comparação na aplicação.

**D23. Detecção de reuso revoga a sessão (família), não o usuário.** As outras
sessões continuam válidas. Sem janela de tolerância por enquanto (documentado).

**D24. Logout revoga a sessão; o JWT de acesso vale até o `exp`.** Tokens de
acesso são stateless e curtos (15 min por padrão). Checar revogação a cada
request custaria uma ida ao banco a mais.

**D25. Argon2id com os parâmetros padrão do RustCrypto (m=19 MiB, t=2, p=1).**
Roda em `spawn_blocking`, com no máximo `min(núcleos, 4)` hashes simultâneos
(pico < 80 MiB). Login de email inexistente verifica contra um hash fictício,
para o tempo de resposta não revelar se a conta existe.

**D26. Rate limit em memória, janela fixa de 1 min**, por IP (signup/token) e
por email (login). Um binário por instalação dispensa Redis. O IP vem do
`X-Forwarded-For` (entrada mais à direita) só com `NELCOTA_TRUST_PROXY=true`.

**D27. Cadastro com email duplicado responde 409.** Sem confirmação de email no
MVP, esconder a existência da conta no cadastro não traria proteção real
(o login continua sem revelar).

**D28. Libs novas.** `argon2` (estava na tabela); `sha2`, `base64` e `getrandom`
já eram dependências transitivas (`jsonwebtoken`/RustCrypto) e foram usadas
diretamente nas mesmas versões, sem crescer a árvore. Licença ISC liberada no
`cargo deny` (vem do `simple_asn1`, usado na leitura de PEM).

## Marco 3

**D29. SQL dinâmico com um builder próprio, sem `sea-query`.** Todo valor da
URL vira parâmetro de **texto**, e o Postgres converte para o tipo da coluna
(`$1::text::<tipo do catálogo>`); inserts e updates usam
`json_populate_recordset`/`json_populate_record`, e RPC usa `json_to_record`.
Assim o Postgres valida os tipos como num literal, e a aplicação não precisa
mapear tipos. O `sea-query` amarra cada valor a um tipo Rust (e o
`sea-query-postgres` exigiria esse mapeamento), além de não expressar bem esses
padrões. O builder tem ~400 linhas com três regras auditáveis: identificador só
do catálogo e sempre entre aspas; tipo de cast só do catálogo; valor sempre
parâmetro. Testes unitários e de integração cobrem injeção nos dois lados.

**D30. OpenAPI montado com `serde_json`, sem `utoipa`.** O `utoipa` gera
especificação em tempo de compilação a partir de tipos Rust; a nossa vem do
catálogo em tempo de execução. O documento é filtrado pelos privilégios da
role do request (como o modo `follow-privileges` do PostgREST).

**D31. Recarga do catálogo por event trigger + `LISTEN nelcota`.** O trigger
roda em `ddl_command_end` e `sql_drop` (inclui GRANT/REVOKE). O listener
agrupa rajadas (100 ms), reconecta com backoff e recarrega a cada reconexão.
Sem superusuário, a migração segue sem o trigger e a recarga é manual (`NOTIFY`).

**D32. PATCH/DELETE exigem filtro.** Diferente do PostgREST (que permite e
deixa o RLS limitar), recusamos com 400. O custo é um filtro explícito quando
a intenção é mesmo atingir tudo.

**D33. Lotes com chaves diferentes respeitam o DEFAULT.** As linhas são
agrupadas por conjunto de colunas e cada grupo vira um INSERT (CTEs numa
instrução). No PostgREST, sem `Prefer: missing=default`, as colunas ausentes
viram NULL; achamos o DEFAULT o comportamento menos surpreendente.

**D34. `statement_timeout` na role `authenticator`.** `ALTER ROLE anon SET ...`
não tem efeito com `SET ROLE` (o Postgres só aplica as configurações da role no
login). O bootstrap aplica `NELCOTA_STATEMENT_TIMEOUT_SECS` (padrão 10 s) ao
`authenticator`, e o erro vira 504.

**D35. Mantido o padrão do Postgres para `EXECUTE` em funções (PUBLIC).**
Mudar os default privileges esconderia um comportamento do Postgres puro; a
documentação e o painel alertam.

**D36. Libs novas.** `form_urlencoded`, `futures-util` e `bytes` já eram
dependências transitivas (axum/tokio-postgres); `compression-gzip` do
tower-http adiciona `flate2`.

## Marco 4

**D37. Um binário, dois papéis.** `nelcota` sem argumentos (ou `serve`) sobe o
servidor; os subcomandos operam a instalação. No host, `migrate`/`types`/`token`
são repassados ao container `app` (`docker compose exec`), que é quem alcança
o Postgres numa rede `internal: true`; com `NELCOTA_DATABASE_URL` no ambiente,
rodam direto.

**D38. Perfis do Postgres viram `-c chave=valor` no compose.** Os arquivos
`deploy/postgres/profiles/*.conf` são a fonte; o `init` os converte em
argumentos do comando `postgres`. Usar `config_file` substituiria o
`postgresql.conf` inteiro da imagem (e mudaria o diretório padrão do
`pg_hba.conf`).

**D39. Backup por `pg_dump -Fc`; envio ao S3 com a imagem `amazon/aws-cli`.**
Nada a instalar no host e nenhuma implementação própria de assinatura S3
(SigV4). Restore em transação única (`--single-transaction --exit-on-error`).
PITR com WAL-G fica como pendência (dumps diários: RPO de até 24 h).

**D40. Upgrade com rollback = imagem anterior + restore do backup pré-upgrade.**
A versão nova pode ter aplicado migrações internas que a anterior não conhece
(o refinery recusa subir com migração "desconhecida"). Por isso o rollback
também restaura o banco. O teste de aceitação cobre esse caminho.

**D41. Imagem `scratch` com binário musl; healthcheck embutido.** Sem shell nem
curl na imagem: `nelcota healthcheck` faz o `GET /health` por TCP puro.

**D42. Migrações do usuário no formato do refinery (`V<n>__<nome>.sql`)**, em
`nelcota.user_migrations`, separadas das internas (`nelcota.schema_migrations`).
CRLF é normalizado antes do checksum (checkout no Windows não diverge).

**D43. `install.sh` instala o Docker pelo script oficial se ele faltar** (como
root e sem `NELCOTA_SKIP_DOCKER=1`): é o que torna os "3 comandos" possíveis
numa VPS zerada. O checksum SHA-256 do binário é sempre conferido.

**D44. `init` não ativa o firewall sem `--firewall`.** Ativar `ufw` remotamente
com a porta SSH errada tranca o dono fora da máquina. Por padrão só recomenda.

## Marco 5

**D45. Painel com HTML no servidor + ~2 KB de JS próprio, sem HTMX.** As páginas
são formulários comuns; o único trecho interativo (editor SQL) cabe em poucas
linhas de `fetch`. CSP `script-src 'self'` sem `unsafe-inline`. Assets
embutidos com `rust-embed`.

**D46. Login do painel separado dos usuários finais.** Email + hash argon2id no
`.env` (gerados pelo `init`; a senha é mostrada uma vez). Sessões em memória
(reiniciar o servidor desloga o admin), cookie `HttpOnly; SameSite=Strict`
(`Secure` atrás do proxy), rate limit de login e checagem de `Origin`/
`Sec-Fetch-Site` em todo POST (CSRF).

**D47. O painel usa a conexão administrativa.** O admin é o dono do banco, como
no `psql`. As escritas em tabelas reutilizam o construtor de SQL da API
(identificadores do catálogo, valores parametrizados). O editor SQL abre uma
conexão **nova** por execução: `BEGIN` sem `COMMIT` ou `SET ROLE` não
contaminam o pool. O texto do SQL não vai para o log.

**D48. Valores no painel preservam o texto do Postgres** (`RawValue`), sem
passar por `f64`: `numeric(30,10)` aparece e é editado com todas as casas.

### Pendências conhecidas

- Embed de relações, `or=`/`and=` e upsert ficam para depois do MVP.
- PITR com WAL-G (ou pgBackRest) arquivando WAL no S3.
- Instalação sem Docker (systemd): o binário já não depende de Docker, falta o
  `init` gerar as units.
- O workflow de release (binários musl + imagem no GHCR) foi escrito, mas só
  roda quando o repositório estiver no GitHub; o `install.sh` depende dele.
