# JWT e roles

Contrato público. O formato do JWT é nosso e documentado aqui; nenhuma
biblioteca dita esse formato.

## Roles do Postgres

| Role            | Login | Quem usa                         | RLS        |
|-----------------|-------|----------------------------------|------------|
| `authenticator` | sim   | a conexão da API (pool)          | n/a        |
| `anon`          | não   | requests sem `Authorization`     | respeitado |
| `authenticated` | não   | usuários finais logados          | respeitado |
| `service_role`  | não   | backend confiável                | **ignorado** (`BYPASSRLS`) |
| `nelcota_auth`  | não   | interna: o servidor lendo `auth.*` | n/a (nenhum JWT a assume) |

`authenticator` é `NOINHERIT`: sozinho não tem privilégio nenhum. Ele é membro
das outras roles e, em cada request, assume uma delas.

> ⚠️ **Um JWT com `role: service_role` ignora todo o RLS. Ele NUNCA pode ir
> para o frontend, um app mobile ou qualquer lugar fora do seu servidor.**

## O que acontece em cada request

1. A API lê `Authorization: Bearer <jwt>`.
   - Sem o header: o request vira `anon`.
   - Header presente mas inválido (assinatura, `exp`, esquema, `kid` ou role
     desconhecidos): **401**. Nunca vira `anon` silenciosamente.
2. Abre **uma** transação e executa, com parâmetros:
   ```sql
   SELECT set_config('role', $1, true),                -- = SET LOCAL ROLE
          set_config('request.jwt.claims', $2, true);  -- claims completas
   ```
3. Executa a query. O RLS do Postgres decide o que a role enxerga.
4. COMMIT (ou ROLLBACK em caso de erro). Role e claims morrem com a transação.

A API nunca implementa regras de permissão por conta própria.

## Assinatura

- **EdDSA (Ed25519)**, padrão. Chave privada em `NELCOTA_JWT_PRIVATE_KEY`
  (PKCS#8 em PEM, ou o base64 de uma linha). A chave pública é publicada em
  `GET /auth/v1/.well-known/jwks.json` (`kty: OKP`, `crv: Ed25519`, `kid` =
  thumbprint RFC 7638). O header de cada token traz o `kid`.
- **HS256**, modo simples/legado, com `NELCOTA_JWT_SECRET` (≥ 32 caracteres).
  Se houver chave EdDSA, o segredo HS256 só serve para **verificar** tokens
  antigos, o que permite migrar sem derrubar ninguém.

Gerar uma chave: `openssl genpkey -algorithm ed25519` (ou, no Marco 4,
`nelcota init`).

## Claims dos tokens de usuário (emitidos pelo `/auth/v1`)

```json
{
  "iss": "nelcota",
  "aud": "authenticated",
  "sub": "3f1c…-uuid",
  "role": "authenticated",
  "email": "ana@exemplo.com",
  "session_id": "8a2d…-uuid",
  "iat": 1767225600,
  "exp": 1767226500
}
```

## Regras de validação (qualquer emissor)

| Claim  | Obrigatória | Regra |
|--------|-------------|-------|
| `role` | sim | `anon`, `authenticated` ou `service_role`. Qualquer outro valor → 401. |
| `sub`  | só para `authenticated` | uuid do usuário (`auth.uid()`). |
| `exp`  | sim | epoch em segundos; tolerância de 30 s. |
| `iss`, `aud` | não | informativas: não são exigidas na verificação. |

As demais claims são repassadas intactas para `auth.jwt()`. Logout revoga a
sessão e os refresh tokens; o JWT de acesso, que é stateless, vale até o `exp`
(padrão: 15 minutos, `NELCOTA_JWT_EXPIRY_SECS`).

## Trocar de provedor de auth

Como as policies só dependem de `auth.uid()`, `auth.role()` e `auth.jwt()`,
qualquer emissor que produza um JWT com as claims acima funciona. A
verificação fica atrás da trait `JwtVerifier`; aceitar um JWKS externo (Auth0,
Keycloak, Zitadel...) é uma nova implementação dessa trait.

## Funções SQL

```sql
auth.jwt()  -> jsonb  -- claims do request ('{}' fora de um request)
auth.uid()  -> uuid   -- claim sub
auth.role() -> text   -- claim role
```

Exemplo de policy:

```sql
CREATE POLICY dono ON public.todos
    FOR SELECT TO authenticated
    USING (user_id = auth.uid());
```

## Códigos HTTP

| Situação | Status |
|---|---|
| JWT inválido, expirado ou com role desconhecida | 401 `invalid_token` |
| `anon` sem permissão na tabela | 401 |
| `authenticated` sem permissão / violação de policy em escrita | 403 |
| Tabela inexistente | 404 |
