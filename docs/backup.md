# Backup e restore

## Backup

```sh
nelcota -p loja backup            # pg_dump -Fc → projects/loja/backups/nelcota-loja-<data>.dump
nelcota -p loja backup --upload   # e envia ao S3-compatible configurado (s3://<bucket>/loja/)
nelcota backup --all --keep 7     # todos os projetos; mantém os 7 dumps locais mais recentes
```

O primeiro `init` (como root) instala um cron diário às 03:00
(`/etc/cron.d/nelcota-backup`) com `backup --all`, e `--upload` quando há S3
configurado. Um projeto com problema não impede o backup dos outros.

O dump é o formato custom do `pg_dump`: inclui schema, dados, policies RLS,
funções e o schema `auth` (usuários com os hashes argon2id). As roles do
cluster (`anon`, `authenticated`...) não vão no dump, mas são recriadas pelas
migrações do Nelcota em qualquer instalação nova.

### S3-compatible

Qualquer serviço com API S3: AWS S3, Backblaze B2, Cloudflare R2, Wasabi,
MinIO. Variáveis no `host.env` (valem para todos os projetos):

```sh
NELCOTA_BACKUP_S3_ENDPOINT=https://s3.us-west-002.backblazeb2.com
NELCOTA_BACKUP_S3_BUCKET=meus-backups
NELCOTA_BACKUP_S3_ACCESS_KEY=...
NELCOTA_BACKUP_S3_SECRET_KEY=...
NELCOTA_BACKUP_S3_REGION=us-west-002
```

O envio usa a imagem oficial `amazon/aws-cli` (nada a instalar no host).
Configure no bucket a retenção (lifecycle) que você quiser.

> **Guarde o `host.env` e o `.env` de cada projeto em lugar seguro, separados
> do bucket.** Eles não vão no backup (contêm segredos). Sem o `.env` do
> projeto, um restore funciona, mas a chave dos JWTs muda e todos os usuários
> precisam logar de novo (as senhas continuam valendo).

## Restore

```sh
nelcota -p loja restore projects/loja/backups/nelcota-loja-20261006T030000Z.dump
```

O restore para o app, executa `pg_restore --clean --if-exists` em **uma
transação** (se algo falhar, o banco fica como estava) e sobe o app de novo.

### Restore numa VPS nova

```sh
curl -fsSL https://nelcota.dev/install | sh
mkdir -p /opt/nelcota && cd /opt/nelcota
nelcota init api.loja.com --yes
cp /caminho/seguro/loja.env projects/loja/.env   # opcional: mantém a chave dos JWTs
nelcota up
aws s3 cp s3://meus-backups/loja/nelcota-loja-....dump projects/loja/backups/ --endpoint-url ...
nelcota -p loja restore projects/loja/backups/nelcota-loja-....dump --yes
```

O teste de aceitação (`scripts/acceptance.sh --local`) faz backup, escreve
mais dados, restaura e confere que o estado voltou ao do backup.

Ao remover um projeto (`nelcota -p <nome> remove`), um backup final fica em
`archive/`, na pasta do host.

## PITR (point-in-time recovery)

Dumps diários perdem até 24 h de escrita no pior caso. Para recuperar até o
último segundo, a evolução planejada é arquivar o WAL continuamente com
**WAL-G** (ou pgBackRest) no mesmo S3. Isso ainda não é gerado pelo `init`;
está registrado como pendência em [decisoes.md](decisoes.md).
