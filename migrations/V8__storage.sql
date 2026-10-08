-- Nelcota: file storage metadata (schema `storage`). Design in
-- docs/decisions.md D77-D80, usage in docs/storage.md.
--
-- The files themselves live in an object store (local disk or S3); this
-- schema only describes them. Who may read, write or delete a file is decided
-- by RLS policies on `storage.objects`, which start with none: until a policy
-- says otherwise, only `service_role` (BYPASSRLS) reaches any file.
--
-- Internal role `nelcota_storage` (no JWT can assume it, like `nelcota_auth`):
-- the server uses it to read bucket settings before an upload and to find
-- bytes that no row points to any more.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'nelcota_storage') THEN
        CREATE ROLE nelcota_storage NOLOGIN NOINHERIT BYPASSRLS;
    END IF;
END
$$;

GRANT nelcota_storage TO authenticator;

CREATE SCHEMA IF NOT EXISTS storage;
GRANT USAGE ON SCHEMA storage TO anon, authenticated, service_role, nelcota_storage;

CREATE TABLE storage.buckets (
    -- Part of every file URL: lowercase letters, digits, '-' and '_'. The
    -- words the routes use after /storage/v1/object/ cannot be bucket names.
    id                 text        PRIMARY KEY CHECK (id ~ '^[a-z0-9][a-z0-9_-]{0,62}$'
                                                      AND id NOT IN ('public', 'sign', 'list')),
    -- Public buckets are readable by anyone through /storage/v1/object/public/.
    -- Writes still go through the policies.
    public             boolean     NOT NULL DEFAULT false,
    -- Largest file accepted, in bytes (NULL = the server-wide cap).
    file_size_limit    bigint      CHECK (file_size_limit > 0),
    -- Accepted types, exact ('image/png') or by family ('image/*'). NULL = any.
    allowed_mime_types text[],
    created_at         timestamptz NOT NULL DEFAULT now(),
    updated_at         timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE storage.objects (
    id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- A bucket with files cannot be dropped: empty it first.
    bucket_id  text        NOT NULL REFERENCES storage.buckets (id),
    -- Path inside the bucket ('avatars/7f9c.../photo.png'). "C" collation so
    -- prefix listings use the unique index.
    name       text        COLLATE "C" NOT NULL CHECK (length(name) BETWEEN 1 AND 1024),
    owner      uuid        DEFAULT auth.uid(),
    -- Key of the bytes in the store ('<bucket>/<version>'). A new upload to the
    -- same name gets a new version, so readers never see a half-written file.
    version    uuid        NOT NULL UNIQUE,
    size       bigint      NOT NULL CHECK (size >= 0),
    mime_type  text        NOT NULL,
    etag       text        NOT NULL,
    metadata   jsonb       NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT objects_bucket_name_key UNIQUE (bucket_id, name)
);
CREATE INDEX objects_owner_idx ON storage.objects (owner);

ALTER TABLE storage.buckets ENABLE ROW LEVEL SECURITY;
ALTER TABLE storage.objects ENABLE ROW LEVEL SECURITY;

-- The privileges are broad on purpose: the policies narrow them down.
GRANT SELECT ON storage.buckets TO anon, authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE ON storage.buckets TO service_role;
GRANT SELECT, INSERT, UPDATE, DELETE ON storage.objects TO anon, authenticated, service_role;
GRANT SELECT ON storage.buckets, storage.objects TO nelcota_storage;

-- Path helpers for policies, e.g. a folder per user:
--   (storage.foldername(name))[1] = auth.uid()::text
CREATE FUNCTION storage.foldername(name text) RETURNS text[]
    LANGUAGE sql IMMUTABLE STRICT
AS $$
    SELECT (string_to_array(name, '/'))[1:cardinality(string_to_array(name, '/')) - 1]
$$;

CREATE FUNCTION storage.filename(name text) RETURNS text
    LANGUAGE sql IMMUTABLE STRICT
AS $$
    SELECT (string_to_array(name, '/'))[cardinality(string_to_array(name, '/'))]
$$;

-- Lowercase extension without the dot ('' when there is none).
CREATE FUNCTION storage.extension(name text) RETURNS text
    LANGUAGE sql IMMUTABLE STRICT
AS $$
    SELECT coalesce(lower(substring(storage.filename(name) FROM '\.([^.]+)$')), '')
$$;

COMMENT ON TABLE storage.buckets IS 'File buckets. See docs/storage.md.';
COMMENT ON TABLE storage.objects IS 'One row per stored file; RLS policies here decide who reaches each file.';
COMMENT ON FUNCTION storage.foldername(text) IS 'Folders of a path: ''a/b/c.png'' -> {a,b}.';
COMMENT ON FUNCTION storage.filename(text) IS 'Last segment of a path: ''a/b/c.png'' -> c.png.';
COMMENT ON FUNCTION storage.extension(text) IS 'Lowercase extension of a path: ''a/b/c.PNG'' -> png.';
