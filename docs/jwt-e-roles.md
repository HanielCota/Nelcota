# JWT e roles

> Rascunho do Marco 1. O contrato completo (emissão, `aud`, `iss`, refresh
> tokens, JWKS) é fechado no Marco 2.

## Roles do Postgres

| Role            | Login | Quem usa                         | RLS        |
|-----------------|-------|----------------------------------|------------|
| `authenticator` | sim   | a conexão da API (pool)          | n/a        |
| `anon`          | não   | requests sem `Authorization`     | respeitado |
| `authenticated` | não   | usuários finais logados          | respeitado |
| `service_role`  | não   | backend confiável                | **ignorado** (`BYPASSRLS`) |

`authenticator` é `NOINHERIT`: sozinho não tem privilégio nenhum. Ele é membro
das outras três e, em cada request, assume uma delas.

> ⚠️ **A chave/JWT de `service_role` ignora todo o RLS. Ela NUNCA pode ir para
> o frontend, um app mobile ou qualquer lugar fora do seu servidor.**

## O que acontece em cada request

1. A API lê `Authorization: Bearer <jwt>`.
   - Sem o header: o request vira `anon`.
   - Header presente mas inválido (assinatura, `exp`, esquema, role
     desconhecida): **401**. Nunca vira `anon` silenciosamente.
2. Abre **uma** transação e executa, com parâmetros:
   ```sql
   SELECT set_config('role', $1, true),                -- = SET LOCAL ROLE
          set_config('request.jwt.claims', $2, true);  -- claims completas
   ```
3. Executa a query. O RLS do Postgres decide o que a role enxerga.
4. COMMIT (ou ROLLBACK em caso de erro). Role e claims morrem com a transação.

A API nunca implementa regras de permissão por conta própria.

## Claims

| Claim  | Obrigatória | Regra |
|--------|-------------|-------|
| `role` | sim | `anon`, `authenticated` ou `service_role`. Qualquer outro valor → 401. |
| `sub`  | só para `authenticated` | uuid do usuário (`auth.uid()`). |
| `exp`  | sim | epoch em segundos; tolerância de 30 s. |

Demais claims são repassadas intactas para `auth.jwt()`.

Assinatura no Marco 1: HS256 com `NELCOTA_JWT_SECRET` (≥ 32 caracteres).

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
