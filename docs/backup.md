# Backup e restore

## Backup

```sh
nelcota backup               # pg_dump -Fc → backups/nelcota-AAAAMMDDTHHMMSSZ.dump
nelcota backup --upload      # e envia ao S3-compatible configurado
nelcota backup --keep 7      # mantém só os 7 dumps locais mais recentes
```

O `init` (como root) instala um cron diário às 03:00
(`/etc/cron.d/nelcota-backup`), com `--upload` quando há S3 configurado.

O dump é o formato custom do `pg_dump`: inclui schema, dados, policies RLS,
funções e o schema `auth` (usuários com os hashes argon2id). As roles do
cluster (`anon`, `authenticated`...) não vão no dump, mas são recriadas pelas
migrações do Nelcota em qualquer instalação nova.

### S3-compatible

Qualquer serviço com API S3: AWS S3, Backblaze B2, Cloudflare R2, Wasabi,
MinIO. Variáveis no `.env`:

```sh
NELCOTA_BACKUP_S3_ENDPOINT=https://s3.us-west-002.backblazeb2.com
NELCOTA_BACKUP_S3_BUCKET=meus-backups
NELCOTA_BACKUP_S3_ACCESS_KEY=...
NELCOTA_BACKUP_S3_SECRET_KEY=...
NELCOTA_BACKUP_S3_REGION=us-west-002
```

O envio usa a imagem oficial `amazon/aws-cli` (nada a instalar no host).
Configure no bucket a retenção (lifecycle) que você quiser.

> **Guarde o `.env` em lugar seguro, separado do bucket.** Ele não vai no backup
> (contém segredos). Sem ele, um restore funciona, mas a chave dos JWTs muda e
> todos os usuários precisam logar de novo (as senhas continuam valendo).

## Restore

```sh
nelcota restore backups/nelcota-20261006T030000Z.dump
```

O restore para o app, executa `pg_restore --clean --if-exists` em **uma
transação** (se algo falhar, o banco fica como estava) e sobe o app de novo.

### Restore numa VPS nova

```sh
curl -fsSL https://nelcota.dev/install | sh
nelcota init api.seudominio.com --yes
cp /caminho/seguro/.env .env      # opcional: mantém a chave dos JWTs e as senhas
nelcota up
aws s3 cp s3://meus-backups/nelcota-....dump backups/ --endpoint-url ...
nelcota restore backups/nelcota-....dump --yes
```

O teste de aceitação (`scripts/acceptance.sh --local`) faz backup, escreve
mais dados, restaura e confere que o estado voltou ao do backup.

## PITR (point-in-time recovery)

Dumps diários perdem até 24 h de escrita no pior caso. Para recuperar até o
último segundo, a evolução planejada é arquivar o WAL continuamente com
**WAL-G** (ou pgBackRest) no mesmo S3. Isso ainda não é gerado pelo `init`;
está registrado como pendência em [decisoes.md](decisoes.md).
