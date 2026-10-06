# Nelcota

BaaS open source, simples e sem lock-in: **Postgres puro + um binário Rust +
deploy em 5 minutos numa VPS.**

- **O Postgres é o produto.** Schema, policies RLS e funções são SQL puro e
  continuam funcionando sem o Nelcota.
- **A autorização é do RLS.** A API só valida o JWT e assume a role dele
  dentro de uma transação. Ela nunca decide permissão por conta própria.
- **Um binário (~10 MB)** com API REST automática, auth (JWT EdDSA + JWKS,
  argon2id, refresh com rotação), painel e CLI de deploy.
- **Padrões abertos:** JWT/JWKS, PHC argon2id, SQL puro, OpenAPI,
  S3-compatible. Dá para sair levando tudo ([saida.md](docs/saida.md)).

## Deploy (VPS zerada → HTTPS)

```sh
curl -fsSL https://nelcota.dev/install | sh
nelcota init api.seudominio.com
nelcota up
```

Detalhes em [docs/deploy.md](docs/deploy.md). Backup e restore em
[docs/backup.md](docs/backup.md).

## Uso em 1 minuto

```sql
-- migrations/V1__notas.sql  →  nelcota migrate
CREATE TABLE public.notas (
    id   bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    dono uuid NOT NULL DEFAULT auth.uid(),
    texto text NOT NULL
);
ALTER TABLE public.notas ENABLE ROW LEVEL SECURITY;
CREATE POLICY dono ON public.notas FOR ALL TO authenticated
    USING (dono = auth.uid()) WITH CHECK (dono = auth.uid());
GRANT SELECT, INSERT, UPDATE, DELETE ON public.notas TO authenticated;
```

```sh
# cadastro → JWT
curl -X POST https://api.seudominio.com/auth/v1/signup \
  -H 'content-type: application/json' -d '{"email":"ana@x.com","password":"senha-forte-123"}'

# CRUD sob RLS
curl -X POST https://api.seudominio.com/rest/v1/notas -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -H 'prefer: return=representation' -d '{"texto":"oi"}'
curl "https://api.seudominio.com/rest/v1/notas?texto=ilike.*oi*&order=id.desc" -H "authorization: Bearer $TOKEN"
```

Tipos para o frontend: `nelcota types -o database.ts`. OpenAPI em `/rest/v1/`.
Painel em `/admin/`: editor de tabelas com edição inline, editor SQL com
autocomplete, usuários e policies RLS.

## Desenvolvimento

Requisitos: Rust stable e Docker.

```sh
cargo run -- dev            # Postgres 17 em container + servidor em http://127.0.0.1:8000
cargo test                  # testes de integração sobem Postgres 17 real (testcontainers)
cargo clippy --all-targets -- -D warnings && cargo fmt --all --check
cargo audit && cargo deny check
./scripts/acceptance.sh --local   # init + up + HTTPS + migrate + backup/restore + rollback

# painel (Svelte 5 + shadcn-svelte + Tailwind v4 + CodeMirror)
cd crates/admin/ui && npm install && npm run dev
```

O build do painel (`crates/admin/ui/dist`) é versionado: compilar o binário não
exige Node.

Os testes provam, contra um Postgres real, que um usuário não lê nem altera
dados de outro (em todos os verbos), que JWTs inválidos, expirados ou com role
desconhecida dão 401, que role e claims não vazam entre requests do pool, que
identificadores e valores maliciosos não viram SQL, e que o reuso de refresh
token derruba a sessão.

## Documentação

- [API REST](docs/api.md): filtros, escrita, RPC, OpenAPI, erros
- [JWT e roles](docs/jwt-e-roles.md): o contrato do token e o fluxo JWT → RLS
- [Schema auth](docs/schema-auth.md): tabelas, formato da senha, endpoints
- [Painel](docs/painel.md)
- [Deploy](docs/deploy.md) · [Backup](docs/backup.md) · [Saindo do Nelcota](docs/saida.md)
- [Arquitetura](docs/arquitetura.md) · [Decisões](docs/decisoes.md) · [Benchmark](bench/README.md)

> **Atenção:** Um JWT com `role: service_role` ignora todo o RLS. Nunca o exponha no
> frontend.

## Fora do MVP

Realtime, Storage (use qualquer S3), Edge Functions, OAuth/MFA/magic link,
multi-tenant, embed de relações. A arquitetura deixa espaço para eles.

## Licença

[Apache-2.0](LICENSE)
