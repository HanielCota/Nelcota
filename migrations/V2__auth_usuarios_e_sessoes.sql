-- Nelcota: usuários, sessões e refresh tokens (schema `auth`).
-- Contrato documentado em docs/schema-auth.md.
--
-- As tabelas de `auth` NÃO são expostas pela API REST. Quem as lê e escreve é
-- o próprio servidor, assumindo a role interna `nelcota_auth` (que nenhum JWT
-- pode assumir: a API só aceita anon/authenticated/service_role).

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'nelcota_auth') THEN
        CREATE ROLE nelcota_auth NOLOGIN NOINHERIT;
    END IF;
END
$$;

GRANT nelcota_auth TO authenticator;
GRANT USAGE ON SCHEMA auth TO nelcota_auth;

CREATE TABLE auth.users (
    id                 uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    email              text        NOT NULL CHECK (email = lower(email) AND length(email) <= 254),
    -- Hash no formato PHC (ex.: $argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>).
    -- NULL = usuário sem senha (reservado para OAuth/magic link no futuro).
    encrypted_password text,
    -- Ponto de extensão: confirmação de email (fora do MVP; hoje sempre NULL).
    email_confirmed_at timestamptz,
    raw_user_meta_data jsonb       NOT NULL DEFAULT '{}'::jsonb,
    created_at         timestamptz NOT NULL DEFAULT now(),
    updated_at         timestamptz NOT NULL DEFAULT now(),
    last_sign_in_at    timestamptz,
    CONSTRAINT users_email_key UNIQUE (email)
);

-- Uma sessão por login. Cada sessão tem uma "família" de refresh tokens.
CREATE TABLE auth.sessions (
    id           uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      uuid        NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
    created_at   timestamptz NOT NULL DEFAULT now(),
    refreshed_at timestamptz,
    -- Preenchido em logout ou quando um refresh token é reutilizado.
    revoked_at   timestamptz,
    user_agent   text,
    ip           inet
);
CREATE INDEX sessions_user_id_idx ON auth.sessions (user_id);

-- Refresh tokens opacos. Só o SHA-256 do token é guardado.
-- Rotação: cada uso marca o token como `revoked` e emite outro na mesma
-- sessão. Usar um token já revogado revoga a sessão inteira (detecção de reuso).
CREATE TABLE auth.refresh_tokens (
    id         bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    session_id uuid        NOT NULL REFERENCES auth.sessions (id) ON DELETE CASCADE,
    token_hash bytea       NOT NULL,
    revoked    boolean     NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    CONSTRAINT refresh_tokens_token_hash_key UNIQUE (token_hash)
);
CREATE INDEX refresh_tokens_session_id_idx ON auth.refresh_tokens (session_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON auth.users, auth.sessions, auth.refresh_tokens TO nelcota_auth;

COMMENT ON TABLE auth.users IS 'Usuários finais. Senha em PHC argon2id. Ver docs/schema-auth.md.';
COMMENT ON TABLE auth.sessions IS 'Uma linha por login; revoked_at encerra a sessão e todos os seus refresh tokens.';
COMMENT ON TABLE auth.refresh_tokens IS 'Refresh tokens opacos (só o SHA-256), com rotação e detecção de reuso.';
