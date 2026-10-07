# Deploy

**VPS zerada → HTTPS em 3 comandos.**

```sh
curl -fsSL https://nelcota.dev/install | sh     # binário (com checksum) + Docker se faltar
mkdir -p /opt/nelcota && cd /opt/nelcota
nelcota init api.seudominio.com                 # cria o host e o primeiro projeto
nelcota up                                      # sobe projetos + Caddy, espera o healthcheck
```

Antes, crie um registro DNS **A** do domínio apontando para o IP da VPS. O
Caddy emite o certificado sozinho no primeiro acesso.

Sem `curl | sh`: baixe `nelcota-<arquitetura>` e `nelcota-<arquitetura>.sha256`
da página de releases, rode `sha256sum -c` e copie para `/usr/local/bin`. O
[script](../scripts/install.sh) faz só isso.

## Host e projetos

Uma VPS pode ter **vários projetos**, cada um isolado: Postgres, usuários,
chaves de JWT, painel e backups próprios. Um projeto só é simplesmente um host
com um projeto.

```
Internet ──80/443──▶ Caddy (compartilhado)
                        ├── api.loja.com ──▶ app-loja ──▶ Postgres da loja   (rede interna própria)
                        └── blog.exemplo.com ──▶ app-blog ──▶ Postgres do blog
```

```
/opt/nelcota/                   (a pasta do host; -C <pasta> ou NELCOTA_ROOT)
├── nelcota-host.json           registro: modo de login, domínio base, projetos (sem segredos)
├── host.env                    segredos do host (0600): admin, segredo de SSO, S3
├── shared/projects.json        lista pública (nome + URL), montada nos apps para o seletor
├── caddy/                      Caddy compartilhado; o Caddyfile é gerado do registro
├── archive/                    backups finais de projetos removidos
└── projects/<nome>/            docker-compose.yml, .env (0600), migrations/, backups/
```

- Só o Caddy publica portas. O Postgres de cada projeto fica numa rede
  `internal: true` exclusiva; os apps falam com o Caddy pela rede `nelcota_edge`.
- Um Postgres por projeto, e não um compartilhado: as roles do Postgres
  (`anon`, `authenticated`, `authenticator`...) valem para o servidor inteiro, e
  separar os servidores mantém o isolamento físico e o `pg_dump` de saída por
  projeto.

### Domínio próprio ou subdomínio

```sh
nelcota init api.loja.com                                # domínio próprio → projeto "loja"
nelcota init api.loja.com --project vendas               # nome explícito
nelcota init --project blog --base-domain exemplo.com    # subdomínio: blog.exemplo.com
nelcota init --project painel                            # usa o domínio base já salvo no host
nelcota init --local --project loja                      # local: https://loja.localhost
```

O nome do projeto vem do domínio (`api.loja.com` → `loja`), ou de `--project`.
Ele aparece no seletor do painel e no `-p` dos comandos.

## Comandos

Com um projeto só, `-p` é opcional. Com vários, os comandos que agem num
projeto pedem `-p <nome>`.

| Comando | O que faz |
|---|---|
| `nelcota projects` | lista os projetos, estado e versão |
| `nelcota up` | sobe todos os projetos e o Caddy (`-p` para um só) |
| `nelcota status` | estado dos projetos |
| `nelcota -p loja logs -f [app]` | logs |
| `nelcota -p loja down` / `down --all` | para (`--volumes` APAGA os dados) |
| `nelcota -p loja migrate` | aplica `projects/loja/migrations/V<n>__<nome>.sql` |
| `nelcota -p loja types -o database.ts` | tipos TypeScript do schema |
| `nelcota -p loja token service-role` | JWT de serviço (**ignora o RLS**) |
| `nelcota -p loja backup [--upload]` / `backup --all` | dump em `backups/` (e no S3) |
| `nelcota -p loja restore <arquivo>` | restaura um dump |
| `nelcota -p loja upgrade` / `upgrade --all` | atualiza com backup e rollback automático |
| `nelcota -p loja remove` | backup final em `archive/`, remove containers, dados e o site do Caddy |
| `nelcota panel-login shared` / `per-project` | login dos painéis: único ou por projeto |
| `nelcota admin-password` | senha nova do painel (`-p` no login por projeto) |

Num host, `migrate`, `types` e `token` rodam dentro do container `app` do
projeto, que é quem alcança o Postgres. Fora de um host, usam
`NELCOTA_DATABASE_URL`.

## Login dos painéis

- **Único** (padrão): um email e uma senha para todos os painéis do host.
  Entrar num painel e trocar de projeto pelo seletor não pede senha de novo.
- **Por projeto**: cada painel com credenciais próprias; o seletor só leva ao
  outro painel, que pede o login dele.

```sh
nelcota panel-login per-project     # gera e mostra uma senha por projeto
nelcota panel-login shared          # volta ao login único (senha do host)
```

Detalhes de segurança em [painel.md](painel.md).

## O que o primeiro `init` faz

1. **Checa** Docker e o plugin compose, portas 80/443 livres, RAM e o DNS do
   domínio.
2. **Gera** os segredos do host: credenciais do admin (senha mostrada **uma
   vez**; o `host.env` guarda só o hash argon2id) e o segredo do login único.
3. **Backup:** pergunta o destino S3-compatible (ou use `--s3-*`) e, como
   root, instala o cron diário de todos os projetos (03:00, guarda 7).
4. **Máquina:** com `--firewall`, configura o `ufw` (SSH, 80 e 443).

Cada `init` seguinte só cria o projeto: segredos próprios (Postgres,
`authenticator`, chave Ed25519), perfil do Postgres pela RAM dividida entre
os projetos, e o site no Caddy (recarregado sem derrubar os outros).

| VPS | Projetos pequenos (estimativa) |
|---|---|
| 1 GB | 2–3 |
| 2 GB | 5–8 |
| 4 GB | 12–20 |

## Migrações

SQL puro em `projects/<nome>/migrations/`, nomeado `V1__criar_tabelas.sql`,
`V2__indices.sql`... Cada arquivo roda numa transação. O controle fica em
`nelcota.user_migrations` (versão, nome, data, checksum): se um arquivo já
aplicado for editado, o `migrate` recusa. Depois de aplicar, a API recarrega o
catálogo sozinha.

### Alterações feitas pelo painel

Tabelas, colunas e policies criadas pelo painel vão direto para o banco do
projeto, mas não existem em outro ambiente (staging, outra máquina, um banco
novo) até virarem arquivo. O painel registra cada uma e lista as pendentes em
**Migrações**; "Gerar migração" baixa `V<n>__<nome>.sql` com elas e já o
registra como aplicado neste banco, com o mesmo checksum que o `migrate`
calcula. Coloque o arquivo em `migrations/` e faça commit: aqui nada roda de
novo, e nos outros ambientes o `migrate` aplica.

O número escolhido passa de tudo que existe no banco e na pasta (o container
lê `./migrations`, montada em `/migrations`), então um arquivo ainda não
aplicado não é atropelado. Fora do container, aponte a pasta com
`NELCOTA_MIGRATIONS_DIR`. O que é feito pelo editor SQL não entra no registro:
copie esse SQL para uma migração à mão.

## Upgrade

`nelcota -p loja upgrade` faz backup, troca `NELCOTA_VERSION` no `.env` do
projeto, baixa a imagem e espera o healthcheck. Se a versão nova não ficar
saudável, volta a versão anterior **e** restaura o backup (a versão nova pode
ter migrado o schema). Os outros projetos não são tocados; `upgrade --all`
atualiza um por vez.

## Desenvolvimento local

```sh
nelcota dev        # Postgres 17 em container (127.0.0.1:54322) + servidor em :8000
```

Para testar o deploy completo localmente (vários projetos, Caddy, HTTPS com
certificado interno): `nelcota init --local --project loja` e abra
`https://loja.localhost/admin/`.

## Teste de aceitação

```sh
ACCEPT_DOMAIN=api.exemplo.com ./scripts/acceptance.sh   # VM real
./scripts/acceptance.sh --local                          # simulação local
```

Última execução local (2026-10-06, Windows 11 + Docker Desktop, imagens em
cache): **dois projetos com HTTPS em 28 s**. Também passaram: migrate
(idempotente), signup e RLS via Caddy, isolamento entre projetos (o JWT de um
não vale no outro; as tabelas de um não existem no outro), login único entre os
painéis com token de uso único, troca para login por projeto e de volta,
`types`, backup → escrita → restore, rollback automático de upgrade sem afetar
o outro projeto e remoção de projeto com backup em `archive/`.

O modo VM ainda não foi executado: depende do repositório publicado e do
primeiro release.
