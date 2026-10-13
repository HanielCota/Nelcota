-- Nelcota: sign-in through external providers (OAuth/OIDC).
-- Contract documented in docs/schema-auth.md.

-- One row per provider account linked to a user. The provider's account id
-- (`sub` in OIDC) identifies it: the email may change on the provider's side.
CREATE TABLE auth.identities (
    id              uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         uuid        NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
    provider        text        NOT NULL,
    provider_id     text        NOT NULL,
    -- The email the provider reported at the last sign-in (informative).
    email           text,
    -- The provider's profile as received (name, picture...).
    identity_data   jsonb       NOT NULL DEFAULT '{}',
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),
    last_sign_in_at timestamptz,
    UNIQUE (provider, provider_id)
);

CREATE INDEX identities_user_idx ON auth.identities (user_id);

-- A sign-in in progress, from `/authorize` to the app's code exchange.
-- Lives minutes. Tokens are stored only as SHA-256, as everywhere in `auth`.
CREATE TABLE auth.flow_states (
    id                bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    -- SHA-256 of the `state` sent to the provider.
    state_hash        bytea       NOT NULL UNIQUE,
    provider          text        NOT NULL,
    -- PKCE verifier toward the provider: needed in the clear to redeem its code.
    provider_verifier text        NOT NULL,
    -- The app's PKCE challenge (S256): the code it receives is useless without
    -- the verifier it kept.
    code_challenge    text        NOT NULL,
    redirect_to       text        NOT NULL,
    -- Set when the provider sends the person back: the code handed to the app.
    auth_code_hash    bytea       UNIQUE,
    user_id           uuid        REFERENCES auth.users (id) ON DELETE CASCADE,
    created_at        timestamptz NOT NULL DEFAULT now(),
    expires_at        timestamptz NOT NULL
);

GRANT SELECT, INSERT, UPDATE, DELETE ON auth.identities, auth.flow_states TO nelcota_auth;
