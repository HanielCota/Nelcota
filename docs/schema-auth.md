# Schema `auth`

Contrato público e estável. Todas as tabelas são SQL comum: dá para ler,
exportar e migrar sem o Nelcota (ver [saida.md](saida.md)).

As tabelas **não** são expostas pela API REST. O servidor as acessa com a role
interna `nelcota_auth`, que nenhum JWT pode assumir. `anon`, `authenticated` e
`service_role` não têm GRANT nelas (há teste para isso).

## `auth.users`

| Coluna | Tipo | Notas |
|---|---|---|
| `id` | `uuid` PK | `gen_random_uuid()`; é o `sub` do JWT e o `auth.uid()` |
| `email` | `text` único | sempre minúsculo, ≤ 254 caracteres |
| `encrypted_password` | `text` | **PHC string argon2id** (ver abaixo); `NULL` = sem senha |
| `email_confirmed_at` | `timestamptz` | ponto de extensão (confirmação de email fora do MVP) |
| `raw_user_meta_data` | `jsonb` | campo `data` do cadastro; devolvido como `user_metadata` |
| `created_at`, `updated_at` | `timestamptz` | |
| `last_sign_in_at` | `timestamptz` | atualizado em cada login |

### Formato da senha

```
$argon2id$v=19$m=19456,t=2,p=1$<salt base64>$<hash base64>
```

É o formato PHC padrão. Os parâmetros (19 MiB, 2 iterações, paralelismo 1)
seguem a recomendação da OWASP. Funciona direto em qualquer biblioteca argon2:
Python `argon2-cffi` (`PasswordHasher().verify(hash, senha)`), Node `argon2`
(`argon2.verify(hash, senha)`), Go `alexedwards/argon2id`, PHP
`password_verify()`, Keycloak/Authentik via import.

## `auth.sessions`

Uma linha por login.

| Coluna | Tipo | Notas |
|---|---|---|
| `id` | `uuid` PK | vai no JWT como claim `session_id` |
| `user_id` | `uuid` → `auth.users` | `ON DELETE CASCADE` |
| `created_at`, `refreshed_at` | `timestamptz` | |
| `revoked_at` | `timestamptz` | logout ou reuso de refresh token detectado |
| `user_agent`, `ip` | `text`, `inet` | informativos |

## `auth.refresh_tokens`

| Coluna | Tipo | Notas |
|---|---|---|
| `id` | `bigint` PK | |
| `session_id` | `uuid` → `auth.sessions` | a "família" do token |
| `token_hash` | `bytea` único | **SHA-256** do token opaco; o token em si nunca é guardado |
| `revoked` | `boolean` | `true` depois de usado (rotação) |
| `created_at`, `expires_at` | `timestamptz` | validade padrão: 30 dias |

### Rotação e detecção de reuso

1. `POST /auth/v1/token?grant_type=refresh_token` com o token R1.
2. R1 válido → marcado `revoked = true`; R2 é emitido na mesma sessão.
3. Se R1 for usado **de novo** (alguém tem uma cópia), a sessão inteira é
   revogada: R2, R3... deixam de valer. Outras sessões do usuário continuam.

Dois refreshes simultâneos com o mesmo token (duas abas) também disparam a
detecção. O cliente deve serializar o refresh. Uma janela de tolerância pode
entrar depois, se necessário.

## Endpoints

| Método e rota | Corpo | Resposta |
|---|---|---|
| `POST /auth/v1/signup` | `{email, password, data?}` | 201 + sessão |
| `POST /auth/v1/token?grant_type=password` | `{email, password}` | 200 + sessão |
| `POST /auth/v1/token?grant_type=refresh_token` | `{refresh_token}` | 200 + sessão |
| `POST /auth/v1/logout` | Bearer | 204 (revoga a sessão) |
| `GET /auth/v1/user` | Bearer | dados do usuário |
| `GET /auth/v1/.well-known/jwks.json` | - | JWKS público |

Sessão:

```json
{
  "access_token": "<JWT>",
  "token_type": "bearer",
  "expires_in": 900,
  "expires_at": 1767225600,
  "refresh_token": "<opaco>",
  "user": { "id": "...", "email": "...", "user_metadata": {}, "created_at": "...", "last_sign_in_at": "...", "email_confirmed_at": null }
}
```

Erros: `422 validation_failed` (email ou senha fora das regras), `409
user_already_exists`, `400 invalid_grant` (credenciais ou refresh inválidos, com
a mesma mensagem para email inexistente e senha errada), `429 rate_limited` com
`Retry-After`, `403 signup_disabled`.

Senha: de 8 a 256 caracteres. Rate limit: `NELCOTA_AUTH_RATE_LIMIT_PER_MINUTE`
por IP (padrão 30) e o mesmo limite por email no login.
