-- Nelcota: tokens de uso único do schema `auth` (hoje, recuperação de senha).
-- Contrato documentado em docs/schema-auth.md.
--
-- Como em `auth.refresh_tokens`, só o SHA-256 do token é guardado: quem lê o
-- banco (ou um backup) não consegue usar um link enviado por email.

CREATE TABLE auth.one_time_tokens (
    id          bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id     uuid        NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
    -- Para que serve o token. Novos tipos (confirmação de email, magic link)
    -- entram ampliando o CHECK.
    kind        text        NOT NULL CHECK (kind IN ('recovery')),
    token_hash  bytea       NOT NULL UNIQUE,
    created_at  timestamptz NOT NULL DEFAULT now(),
    expires_at  timestamptz NOT NULL,
    -- Preenchido quando o token é usado: não vale de novo.
    used_at     timestamptz
);

CREATE INDEX one_time_tokens_user_kind_idx ON auth.one_time_tokens (user_id, kind);

GRANT SELECT, INSERT, UPDATE, DELETE ON auth.one_time_tokens TO nelcota_auth;
