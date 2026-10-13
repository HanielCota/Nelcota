-- Alterações de schema feitas pelo painel (/admin) e as migrações geradas a
-- partir delas.
--
-- O painel cria tabelas, colunas e policies direto no banco, mas o deploy
-- confia nos arquivos de `migrations/`. Cada DDL aplicado pelo painel fica
-- registrado aqui (na mesma transação do DDL); "Gerar migração" junta os
-- pendentes num arquivo `V<n>__<nome>.sql` e o registra como já aplicado em
-- `nelcota.user_migrations`. Schema interno: nenhuma role da API o enxerga.

CREATE SCHEMA IF NOT EXISTS nelcota;

-- Migrações geradas pelo painel. O texto é guardado para o arquivo poder ser
-- baixado de novo idêntico (o checksum depende de cada byte).
CREATE TABLE nelcota.panel_migrations (
    version    integer     PRIMARY KEY,
    filename   text        NOT NULL,
    sql        text        NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE nelcota.panel_changes (
    id          bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    applied_at  timestamptz NOT NULL DEFAULT now(),
    -- Mensagem que o painel mostrou (ex.: "tabela pedidos criada").
    summary     text        NOT NULL,
    statements  text[]      NOT NULL,
    -- Migração que contém esta alteração; NULL = ainda fora das migrações.
    exported_in integer     REFERENCES nelcota.panel_migrations (version)
);

CREATE INDEX panel_changes_pending_idx ON nelcota.panel_changes (id) WHERE exported_in IS NULL;
