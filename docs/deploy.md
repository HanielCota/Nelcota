# Deploy

**VPS zerada → HTTPS em 3 comandos.**

```sh
curl -fsSL https://nelcota.dev/install | sh     # binário (com checksum) + Docker se faltar
nelcota init api.seudominio.com                 # checa, gera segredos e arquivos
nelcota up                                      # postgres + app + caddy, espera o healthcheck
```

Antes, crie um registro DNS **A** de `api.seudominio.com` apontando para o IP
da VPS. O Caddy emite o certificado sozinho no primeiro acesso.

Sem `curl | sh`: baixe `nelcota-<arquitetura>` e `nelcota-<arquitetura>.sha256`
da página de releases, rode `sha256sum -c` e copie para `/usr/local/bin`. O
[script](../scripts/install.sh) faz só isso e tem 80 linhas.

## O que o `init` faz

1. **Checa:** Docker e o plugin compose, portas 80/443 livres, RAM, e se o DNS do
   domínio aponta para o IP público da máquina.
2. **Escolhe o perfil do Postgres** pela RAM (`1gb`, `2gb`, `4gb` ou `8gb`;
   veja `deploy/postgres/profiles/`). Para forçar um: `--profile 2gb`.
3. **Gera segredos:** senha do Postgres, senha do `authenticator`, chave
   Ed25519 dos JWTs e senha do admin do painel (mostrada **uma vez**; o
   `.env` guarda só o hash argon2id).
4. **Escreve** `docker-compose.yml`, `Caddyfile`, `.env` (permissão 600),
   `.gitignore`, `migrations/` e `backups/`.
5. **Backup:** pergunta o destino S3-compatible (ou use as flags `--s3-*`) e,
   como root, instala o cron diário (`/etc/cron.d/nelcota-backup`, 03:00, guarda 7).
6. **Máquina:** com `--firewall`, configura o `ufw` (SSH, 80 e 443). Também
   recomenda `unattended-upgrades` se não estiver instalado.

Flags úteis: `--yes` (sem perguntas), `--local` (sem domínio, HTTPS em
`https://localhost` com certificado interno), `--email`, `--image`, `--version`.

## Arquitetura no servidor

```
Internet ──80/443──▶ caddy ──(rede web)──▶ app:8000 ──(rede interna)──▶ postgres:5432
```

- Só o Caddy publica portas. A rede `interno` é `internal: true`: o Postgres
  não sai para a internet nem é alcançável de fora.
- `restart: unless-stopped` e healthchecks em todos os serviços.
- O app é um binário estático numa imagem `scratch` (~10 MB).

## Comandos do dia a dia

| Comando | O que faz |
|---|---|
| `nelcota up` / `down` | sobe/para (`down --volumes` APAGA os dados) |
| `nelcota status` | containers + healthcheck da API |
| `nelcota logs -f [app]` | logs |
| `nelcota migrate` | aplica `migrations/V<n>__<nome>.sql` |
| `nelcota types -o database.ts` | tipos TypeScript do schema |
| `nelcota token service-role` | JWT de serviço (**ignora o RLS**) |
| `nelcota backup [--upload]` | dump em `backups/` (e no S3) |
| `nelcota restore <arquivo>` | restaura um dump |
| `nelcota upgrade [--version X]` | atualiza com backup e rollback automático |
| `nelcota admin-password` | senha nova para o painel |

Num projeto do `init`, `migrate`, `types` e `token` rodam dentro do container
`app`, que é quem alcança o Postgres. Fora dele, usam `NELCOTA_DATABASE_URL`.

## Migrações

SQL puro em `migrations/`, nomeado `V1__criar_tabelas.sql`,
`V2__indices.sql`... Cada arquivo roda numa transação. O controle fica em
`nelcota.user_migrations` (versão, nome, data, checksum): se um arquivo já
aplicado for editado, o `migrate` recusa (checksum divergente). Depois de
aplicar, a API recarrega o catálogo sozinha.

Sem o Nelcota, as migrações continuam aplicáveis com
`psql -f migrations/V1__....sql`.

## Upgrade

`nelcota upgrade` faz, em ordem:

1. backup (`backups/nelcota-<data>.dump`);
2. troca `NELCOTA_VERSION` no `.env`, `docker compose pull app` e `up -d app`;
3. espera o healthcheck (até 2 min).

Se a versão nova não ficar saudável, ele **volta sozinho**: restaura a versão
anterior no `.env`, restaura o banco a partir do backup do passo 1 (a versão
nova pode ter migrado o schema) e sobe o app de novo. O teste de aceitação
cobre esse caminho.

O binário do host é atualizado rodando o instalador de novo.

## Desenvolvimento local

```sh
nelcota dev        # Postgres 17 em container (127.0.0.1:54322) + servidor em :8000
```

Os segredos de dev ficam em `.nelcota/dev.env` (fora do git). `nelcota migrate`
e `nelcota types` funcionam contra esse ambiente.

## Sem Docker (futuro)

O binário não depende de Docker: lê config por variável de ambiente, escuta numa
porta e aplica as próprias migrações. Uma unit systemd + Postgres do sistema +
Caddy do sistema funcionam hoje; o `init` só ainda não gera esses arquivos.

## Teste de aceitação

```sh
ACCEPT_DOMAIN=api.exemplo.com ./scripts/acceptance.sh   # VM real
./scripts/acceptance.sh --local                          # simulação local
```
