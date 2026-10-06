# Painel

Em `https://<seu-domínio>/admin/`. Login próprio, separado dos usuários finais:
email e senha gerados pelo `nelcota init` (a senha aparece uma única vez).

Visual de console de banco: tema escuro por padrão (claro se o sistema estiver
em modo claro), sidebar com seções, editor de tabelas com lista lateral e grade
de dados, fontes Inter e JetBrains Mono embutidas. Tudo é servido pelo próprio
binário: sem CDN e sem build de frontend.

| Página | O que tem |
|---|---|
| **Visão geral** | contadores (tabelas, usuários, policies, funções), alerta de tabelas sem RLS e lista de tabelas com GRANTs |
| **Editor de tabelas** | todas as tabelas do schema exposto, linhas estimadas, GRANTs de `anon`/`authenticated` e status do RLS. Listar, inserir, editar e apagar linhas (tabelas com chave primária) |
| **SQL** | editor que roda como o dono do banco (ignora RLS); Ctrl+Enter executa; erros com código e posição |
| **Usuários** | busca por email, último login, sessões ativas; encerrar sessões ou apagar usuário |
| **Policies** | policies de cada tabela (`USING`/`WITH CHECK`), tabelas sem RLS, funções executáveis por `anon` |

## Alertas de RLS

- **⚠ sem RLS**: tabela comum com GRANT para `anon` ou `authenticated` e RLS
  desligado. Quem tem o GRANT vê e altera **todas** as linhas. Aparece em
  destaque no topo do painel e na página de policies.
- **RLS sem policies**: RLS ligado e nenhuma policy. Ninguém além de
  `service_role` (e do dono) acessa: é seguro, mas talvez não seja o que você
  quer.

## Trocar a senha do admin

```sh
nelcota admin-password      # gera senha nova, grava só o hash no .env e reinicia o app
```

Também dá para usar um hash argon2id (formato PHC) gerado por qualquer
ferramenta: `NELCOTA_ADMIN_PASSWORD_HASH='$argon2id$...'` no `.env` (entre
aspas simples por causa dos `$`), seguido de `docker compose up -d --force-recreate app`.

## Segurança

- Cookie de sessão `HttpOnly`, `SameSite=Strict` e `Secure` atrás do Caddy;
  sessões em memória com validade de 12 h (reiniciar o servidor desloga).
- Todo POST precisa vir da mesma origem (`Origin`/`Sec-Fetch-Site`).
- CSP sem scripts inline, `X-Frame-Options: DENY`, `no-store`.
- Tentativas de login limitadas a 10 por minuto.
- O editor SQL usa uma conexão nova por execução e o texto do SQL não vai para
  o log.
- Sem `NELCOTA_ADMIN_EMAIL` e `NELCOTA_ADMIN_PASSWORD_HASH`, o painel fica
  desligado (as rotas `/admin` nem existem).
