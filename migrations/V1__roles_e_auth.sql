-- Nelcota: roles da API e funções de contexto do request.
--
-- Este arquivo é SQL puro e não depende do binário do Nelcota. Se o projeto
-- sumir, as roles e funções continuam funcionando no Postgres.
--
-- Modelo (documentado em docs/jwt-e-roles.md):
--   authenticator  LOGIN NOINHERIT: a única role com que a API conecta. Sozinha
--                  não tem privilégio nenhum; em cada request a API faz
--                  `set_config('role', <role do JWT>, true)` dentro da transação.
--   anon           requests sem JWT.
--   authenticated  usuários finais logados (JWT com role = authenticated).
--   service_role   backend confiável; BYPASSRLS. A chave NUNCA vai para o frontend.
--
-- Roles são globais no cluster, então a criação é idempotente.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
        CREATE ROLE anon NOLOGIN NOINHERIT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
        CREATE ROLE authenticated NOLOGIN NOINHERIT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'service_role') THEN
        CREATE ROLE service_role NOLOGIN NOINHERIT BYPASSRLS;
    END IF;
    -- Sem senha aqui: segredos não vivem em migrações. O servidor define a
    -- senha na inicialização (verificador SCRAM calculado no cliente).
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticator') THEN
        CREATE ROLE authenticator LOGIN NOINHERIT;
    END IF;
END
$$;

GRANT anon, authenticated, service_role TO authenticator;

-- Extensões ficam num schema próprio para não poluírem o `public`
-- (que é o schema exposto pela API).
CREATE SCHEMA IF NOT EXISTS extensions;
GRANT USAGE ON SCHEMA extensions TO anon, authenticated, service_role;
CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA extensions;
CREATE EXTENSION IF NOT EXISTS pg_stat_statements WITH SCHEMA extensions;

-- Schema `auth`: as tabelas de usuários entram no Marco 2. Aqui ficam só as
-- funções que as policies RLS usam.
CREATE SCHEMA IF NOT EXISTS auth;
GRANT USAGE ON SCHEMA auth TO anon, authenticated, service_role;

GRANT USAGE ON SCHEMA public TO anon, authenticated, service_role;

-- Claims completas do JWT do request atual (ou '{}' fora de um request).
CREATE OR REPLACE FUNCTION auth.jwt() RETURNS jsonb
    LANGUAGE sql STABLE
AS $$
    SELECT coalesce(nullif(current_setting('request.jwt.claims', true), ''), '{}')::jsonb
$$;

-- Id do usuário (claim `sub`) ou NULL para anon/service_role sem `sub`.
CREATE OR REPLACE FUNCTION auth.uid() RETURNS uuid
    LANGUAGE sql STABLE
AS $$
    SELECT nullif(auth.jwt() ->> 'sub', '')::uuid
$$;

-- Role do JWT: 'anon', 'authenticated' ou 'service_role'.
CREATE OR REPLACE FUNCTION auth.role() RETURNS text
    LANGUAGE sql STABLE
AS $$
    SELECT nullif(auth.jwt() ->> 'role', '')
$$;

COMMENT ON FUNCTION auth.jwt() IS 'Claims do JWT do request atual (request.jwt.claims).';
COMMENT ON FUNCTION auth.uid() IS 'Claim sub do JWT do request atual, como uuid.';
COMMENT ON FUNCTION auth.role() IS 'Claim role do JWT do request atual.';
