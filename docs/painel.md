# Painel

Em `https://<seu-domínio>/admin/`. Login próprio, separado dos usuários finais:
email e senha gerados pelo `nelcota init` (a senha aparece uma única vez).

SPA em **Svelte 5 + shadcn-svelte + Tailwind v4**, com **CodeMirror 6** no
editor SQL e fontes IBM Plex Sans/Mono. O build fica embutido no binário: sem
CDN e sem Node em produção. Tema escuro por padrão; o claro fica no menu do
usuário (canto superior direito).

Cor aparece só onde há algo a corrigir: tabela exposta sem RLS (vermelho), RLS
ligado sem policies (âmbar), erros e ações destrutivas. O resto é neutro.

| Recurso | Como usar |
|---|---|
| Editar uma célula | duplo clique; Enter salva, Esc cancela, botão NULL para nulos |
| Editar ou inserir linha completa | lápis no fim da linha / "Inserir linha" (painel lateral) |
| Apagar várias linhas | marque as caixas e "Apagar N" (uma transação só) |
| Ordenar | clique no cabeçalho da coluna (asc → desc → sem ordem) |
| Executar SQL | Ctrl+Enter; autocomplete de tabelas e colunas; modelos e histórico |

| Página | O que tem |
|---|---|
| **Visão geral** | contadores (tabelas, usuários, policies, funções), alerta de tabelas sem RLS e lista de tabelas com GRANTs |
| **Editor de tabelas** | todas as tabelas do schema exposto, linhas estimadas, GRANTs de `anon`/`authenticated` e status do RLS. Listar, inserir, editar e apagar linhas (tabelas com chave primária) |
| **SQL** | editor que roda como o dono do banco (ignora RLS); Ctrl+Enter executa; erros com código e posição |
| **Usuários** | busca por email, último login, sessões ativas; encerrar sessões ou apagar usuário |
| **Policies** | policies de cada tabela (`USING`/`WITH CHECK`), tabelas sem RLS, funções executáveis por `anon` |

## Alertas de RLS

- **sem RLS**: tabela comum com GRANT para `anon` ou `authenticated` e RLS
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
- A página em si não contém dados: tudo vem de `/admin/api/*`, que exige a
  sessão. Todo request que muda estado precisa vir da mesma origem.
- CSP sem scripts inline (`script-src 'self'`), `X-Frame-Options: DENY`.
- O Svelte escapa todo texto; o painel nunca usa `{@html}`.

## Desenvolvimento do painel

```sh
cd crates/admin/ui
npm install
npm run dev        # Vite com hot reload; /admin/api é encaminhado ao nelcota em https://localhost
npm run check      # tipos (svelte-check)
npm run build      # gera dist/ (versionado e embutido no binário)
```

Depois do `npm run build`, recompile o binário (`cargo build`) e faça commit do
`dist/` junto com o código. O CI confere que o `dist` está atualizado.
- Tentativas de login limitadas a 10 por minuto.
- O editor SQL usa uma conexão nova por execução e o texto do SQL não vai para
  o log.
- Sem `NELCOTA_ADMIN_EMAIL` e `NELCOTA_ADMIN_PASSWORD_HASH`, o painel fica
  desligado (as rotas `/admin` nem existem).
