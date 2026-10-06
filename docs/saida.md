# Saindo do Nelcota (sem lock-in)

Teste de fogo: **em 10 minutos dá para sair levando tudo.** Tudo o que
importa vive no Postgres, em SQL comum.

## 1. Banco inteiro (schema, dados, policies, funções, usuários)

```sh
docker compose exec -T postgres pg_dump -U postgres -Fc postgres > nelcota.dump
# restaurar em qualquer Postgres 17+:
pg_restore --no-owner -d "postgres://usuario@novo-host/banco" nelcota.dump
```

As roles `anon`, `authenticated`, `service_role`, `authenticator` e
`nelcota_auth` são globais e não entram no `pg_dump`. Crie-as no destino
antes (o arquivo `migrations/V1__roles_e_auth.sql` faz isso e é idempotente)
ou use `pg_restore --no-acl` se não for usá-las.

## 2. Só os usuários (com os hashes)

```sh
docker compose exec -T postgres psql -U postgres -c "\copy (
  SELECT id, email, encrypted_password, email_confirmed_at,
         raw_user_meta_data, created_at, last_sign_in_at
  FROM auth.users) TO STDOUT WITH CSV HEADER" > usuarios.csv
```

`encrypted_password` está no formato PHC argon2id padrão
(`$argon2id$v=19$m=19456,t=2,p=1$...`). Qualquer biblioteca argon2 verifica
esses hashes sem conversão: os usuários **não precisam trocar de senha**.
Detalhes em [schema-auth.md](schema-auth.md).

## 3. Os JWTs

O formato está em [jwt-e-roles.md](jwt-e-roles.md). Para trocar de provedor
de auth sem reescrever as policies, o novo provedor só precisa emitir tokens
com `role` e `sub`. As funções `auth.uid()`/`auth.jwt()` continuam iguais.

## 4. O que fica para trás

Só o binário `nelcota`: a API REST automática e o painel. Sem o Nelcota, o
Postgres continua servindo exatamente os mesmos dados com as mesmas policies,
e você pode apontar PostgREST, Hasura ou o seu próprio backend para ele.
