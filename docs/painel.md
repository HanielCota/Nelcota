# Painel

Em `https://<domínio-do-projeto>/admin/`. Login próprio, separado dos usuários
finais: email e senha gerados pelo primeiro `nelcota init` (a senha aparece uma
única vez).

## Vários projetos

Cada projeto tem o próprio painel, no próprio domínio, mostrando só os dados
dele. No topo da barra lateral, o **seletor de projetos** (nome do projeto
atual) lista os outros projetos do host e leva a "Todos os projetos": nome,
domínio, estado (no ar / fora do ar), versão e um botão para abrir cada painel.

Com o **login único** (padrão), trocar de projeto pelo seletor não pede senha.
Com o **login por projeto** (`nelcota panel-login per-project`), o seletor só
leva ao outro painel, que pede o login dele. A tela de login sempre mostra de
qual projeto é o painel.

### Como funciona o login único

Cookies não atravessam domínios diferentes (`api.loja.com` → `api.blog.com`),
então o painel de origem faz um *handoff*:

1. O admin, já logado no painel da loja, escolhe "blog" no seletor.
2. O painel da loja emite um token para o blog: JWT HS256 assinado com o
   segredo compartilhado do host, `aud` = blog, validade de 60 s, `jti` único.
3. O navegador abre `https://<blog>/admin/#sso=<token>`. O token vai no
   **fragmento** da URL, que não aparece em logs de servidor nem no `Referer`.
4. O painel do blog valida assinatura, destino, validade e o email do admin,
   recusa tokens já usados e cria a própria sessão. O token sai da URL na hora.

No login por projeto não há segredo compartilhado: o handoff deixa de existir,
e cada painel só aceita as próprias credenciais.

SPA em **Svelte 5 + shadcn-svelte + Tailwind v4**, com **CodeMirror 6** no
editor SQL, fonte Manrope na interface e IBM Plex Mono no código. O build fica
embutido no binário: sem CDN e sem Node em produção (as fontes também, por causa
da CSP `font-src 'self'`). Tema escuro por padrão; o botão de tema fica na barra
superior.

O estado do RLS aparece como selo colorido: verde (RLS com policies), âmbar
(RLS ligado sem policies) e vermelho (tabela exposta sem RLS).

| Recurso | Como usar |
|---|---|
| Editar uma célula | duplo clique; Enter salva, Esc cancela, botão NULL para nulos |
| Editar ou inserir linha completa | lápis no fim da linha / "Inserir linha" (painel lateral) |
| Apagar várias linhas | marque as caixas e "Apagar N" (uma transação só) |
| Ordenar | clique no cabeçalho da coluna (asc → desc → sem ordem) |
| Filtrar | "Filtrar" na grade: igual, diferente, contém, maior/menor, é/não é NULL. Os filtros ficam na URL no formato da API REST (`/admin/tables/pedidos?status=eq.pago`): dá para compartilhar o link e voltar com o navegador |
| Exportar tabela | "Exportar" → CSV ou JSON, com a ordem e os filtros da grade. O servidor lê em fluxo (sem limite de linhas, sem carregar tudo na memória); o CSV tem BOM para o Excel reconhecer acentos |
| Seguir chave estrangeira | a seta numa célula de FK abre a linha referenciada na outra tabela; o cabeçalho mostra `→ tabela` |
| Executar SQL | Ctrl+Enter; autocomplete de tabelas e colunas; modelos e histórico |
| Salvar consultas | Ctrl+S ou "Salvar"; ficam na barra lateral do editor e na paleta. Guardadas no navegador, por projeto |
| Exportar resultado do SQL | "Exportar" acima de cada resultado → CSV ou JSON |
| Paleta de comandos | Ctrl+K (⌘K no Mac) ou "Buscar…" na barra superior: páginas, tabelas, consultas salvas, modelos e ações |
| Criar tabela | "+" na lista de tabelas: colunas (tipo, default, PK, identity, UNIQUE, chave estrangeira), RLS ligado por padrão e matriz de GRANTs. Mostra o SQL antes de criar |
| Editar a estrutura | aba **Estrutura** da tabela (`/admin/tables/<nome>/structure`): adicionar, editar e apagar colunas; renomear a tabela; descrição; ligar/desligar o RLS; GRANTs por role; apagar a tabela (digitando o nome) |
| Policies | "Nova policy" em cada tabela, com modelos (leitura pública, logados leem, dono lê/cria/altera/apaga); editar e apagar; "Ativar RLS" nas tabelas sem |
| Token service_role | página **API**: gera um token com validade escolhida; aparece uma vez e não é guardado |
| Criar usuário / redefinir senha | página **Usuários**: "Novo usuário" e "Redefinir senha…" no menu de cada um, com gerador de senha. Mesmas regras do cadastro público. Não há convite nem confirmação de email: o nelcota não envia emails e o login não exige confirmação |
| Datas na grade | `timestamptz` aparece no fuso de quem vê (`06/10/2026, 19:26:15`); `timestamp` e `date` como gravados. O valor exato fica no tooltip, na edição e na exportação |

| Página | O que tem |
|---|---|
| **Visão geral** | contadores (tabelas, usuários, policies, funções), alerta de tabelas sem RLS e lista de tabelas com GRANTs |
| **Editor de tabelas** | todas as tabelas do schema exposto, linhas estimadas, GRANTs de `anon`/`authenticated` e status do RLS. Listar, inserir, editar e apagar linhas (tabelas com chave primária); criar tabelas e editar a estrutura |
| **SQL** | editor que roda como o dono do banco (ignora RLS); Ctrl+Enter executa; erros com código e posição |
| **Usuários** | busca por email, último login, sessões ativas; criar usuário, redefinir senha (encerra as sessões), encerrar sessões ou apagar usuário |
| **Policies** | policies de cada tabela (`USING`/`WITH CHECK`): criar, editar e apagar; tabelas sem RLS; funções executáveis por `anon` |
| **API** | endereço do projeto, como cada role chama a API, exemplos em curl e JavaScript gerados das colunas de cada tabela, token service_role |

## Alertas de RLS

- **sem RLS**: tabela comum com GRANT para `anon` ou `authenticated` e RLS
  desligado. Quem tem o GRANT vê e altera **todas** as linhas. Aparece em
  destaque no topo do painel e na página de policies.
- **RLS sem policies**: RLS ligado e nenhuma policy. Ninguém além de
  `service_role` (e do dono) acessa: é seguro, mas talvez não seja o que você
  quer.

## Trocar a senha do admin

```sh
nelcota admin-password            # login único: senha nova do host, aplicada a todos os projetos
nelcota -p loja admin-password    # login por projeto: senha nova só da loja
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
- DDL feito pelo painel (tabelas, colunas, policies): a interface envia uma
  especificação tipada, e o servidor monta o SQL com identificadores sempre
  entre aspas e tipos de uma lista fechada. Expressões (`DEFAULT`, `USING`,
  `WITH CHECK`) são SQL por natureza: cada comando vai pelo protocolo
  estendido, que recusa um segundo comando escondido na expressão, e tudo roda
  numa transação (ou nada é aplicado). O SQL aparece antes de executar.
- Chaves estrangeiras pelo formulário só apontam para o schema exposto (para
  `auth.users`, use o editor SQL).
- Tokens `service_role` emitidos pelo painel não são guardados nem vão para o
  log (só o fato e a validade).

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
