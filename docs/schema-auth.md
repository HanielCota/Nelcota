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
| `email_confirmed_at` | `timestamptz` | preenchido quando a pessoa usa um link de recuperação (prova que recebe os emails); confirmação no cadastro ainda fora do MVP |
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

## `auth.one_time_tokens`

Links enviados por email (hoje, só recuperação de senha).

| Coluna | Tipo | Notas |
|---|---|---|
| `id` | `bigint` PK | |
| `user_id` | `uuid` → `auth.users` | `ON DELETE CASCADE` |
| `kind` | `text` | `recovery` |
| `token_hash` | `bytea` único | **SHA-256** do token; o token em si nunca é guardado |
| `created_at`, `expires_at` | `timestamptz` | validade: 1 hora |
| `used_at` | `timestamptz` | preenchido no uso; o link não vale de novo |

## Endpoints

| Método e rota | Corpo | Resposta |
|---|---|---|
| `POST /auth/v1/signup` | `{email, password, data?}` | 201 + sessão |
| `POST /auth/v1/token?grant_type=password` | `{email, password}` | 200 + sessão |
| `POST /auth/v1/token?grant_type=refresh_token` | `{refresh_token}` | 200 + sessão |
| `POST /auth/v1/logout` | Bearer | 204 (revoga a sessão) |
| `GET /auth/v1/user` | Bearer | dados do usuário |
| `GET /auth/v1/.well-known/jwks.json` | - | JWKS público |
| `POST /auth/v1/recover` | `{email}` | 200 `{}` (exista ou não a conta) |
| `POST /auth/v1/verify` | `{type: "recovery", token, password}` | 200 + sessão |

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

Recuperação: `403 recovery_disabled` (projeto sem SMTP), `400 invalid_grant`
(link inválido, expirado ou já usado), `400 unsupported_type`.

Senha: de 8 a 256 caracteres. Rate limit: `NELCOTA_AUTH_RATE_LIMIT_PER_MINUTE`
por IP (padrão 30) e o mesmo limite por email no login.

## Recuperação de senha

Desligada até o projeto configurar um SMTP (o Nelcota não tem servidor de
email próprio; use o do seu provedor: Postmark, SES, Resend...). No `.env` do
projeto, os três juntos:

```sh
NELCOTA_SMTP_URL=smtps://usuario:senha@smtp.exemplo.com:465   # ou smtp://...:587?tls=required
NELCOTA_SMTP_FROM=Loja <nao-responda@loja.com>
NELCOTA_PASSWORD_RECOVERY_URL=https://app.loja.com/nova-senha
```

Configuração pela metade impede o servidor de subir (melhor do que descobrir
no primeiro "esqueci minha senha").

1. O app chama `POST /auth/v1/recover {email}`. A resposta é sempre `200 {}`,
   para não revelar quem tem conta. Se a conta existe, chega um email com o
   link `https://app.loja.com/nova-senha#type=recovery&token=...`.
2. Essa página do app lê o token do fragmento (`location.hash`), pede a senha
   nova e chama `POST /auth/v1/verify {type: "recovery", token, password}`.
3. A resposta é uma sessão (mesmo formato do login): a pessoa já entra.

Garantias:

- O token vai no fragmento da URL, que o navegador não envia a nenhum
  servidor: não aparece em logs nem no `Referer`.
- Vale 1 hora e uma vez só. Pedir de novo invalida o link anterior.
- No máximo um email por minuto para a mesma conta, além do rate limit por IP:
  o endpoint não serve para lotar a caixa de entrada de alguém.
- O email sai em segundo plano: o tempo de resposta não revela se a conta
  existe, e uma falha do SMTP vai para o log em vez de virar erro.
- Trocar a senha encerra todas as sessões da conta (os refresh tokens deixam de
  valer na hora; JWTs de acesso já emitidos valem até expirar).
