-- Nelcota: new single-use email links in `auth.one_time_tokens`.
-- Contract documented in docs/schema-auth.md.
--
-- `signup` confirms the email of a new account; `magiclink` signs in without
-- a password. Both reuse the recovery storage: only the token's SHA-256,
-- an expiry and `used_at`.

ALTER TABLE auth.one_time_tokens DROP CONSTRAINT one_time_tokens_kind_check;
ALTER TABLE auth.one_time_tokens ADD CONSTRAINT one_time_tokens_kind_check
    CHECK (kind IN ('recovery', 'signup', 'magiclink'));
