-- Encode parent segments as UTF-8 bytes terminated by NUL. Text cannot contain
-- NUL, so the separator preserves C ordering of names (including a vs a-)
-- and gives each directory subtree an exact, contiguous index range.
CREATE FUNCTION storage.directory_key(name text) RETURNS bytea
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
    SET search_path = pg_catalog
AS $$
    SELECT coalesce(string_agg(convert_to(segment, 'UTF8') || decode('00', 'hex'),
                               ''::bytea ORDER BY position), ''::bytea)
      FROM unnest((string_to_array(name, '/'))[1:cardinality(string_to_array(name, '/')) - 1])
           WITH ORDINALITY AS parents(segment, position)
$$;
-- These helpers only transform caller-provided values, with no table reads or
-- elevated privileges. Public execution preserves writes by custom SQL roles.
GRANT EXECUTE ON FUNCTION storage.directory_key(text) TO PUBLIC;

-- Keep the ordered file index as narrow as the existing (bucket, name) index,
-- including for legacy names written through SQL. A 32-bit fingerprint narrows
-- the seek; the query also checks the exact bucket and directory for collisions.
CREATE FUNCTION storage.directory_hash(bucket_id text, parent_key bytea) RETURNS integer
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
    SET search_path = pg_catalog
AS $$
    SELECT hashtext((bucket_id || ':' || encode(parent_key, 'hex')) COLLATE "C")
$$;
GRANT EXECUTE ON FUNCTION storage.directory_hash(text, bytea) TO PUBLIC;

ALTER TABLE storage.objects
    ADD COLUMN directory_key bytea GENERATED ALWAYS AS (storage.directory_key(name)) STORED,
    -- Index a stored value: a function over row data cannot be pushed ahead of
    -- RLS unless it is leakproof. No elevated function privileges are needed.
    ADD COLUMN directory_scope integer GENERATED ALWAYS AS (
        storage.directory_hash(bucket_id, storage.directory_key(name))) STORED;
CREATE INDEX objects_directory_idx ON storage.objects (bucket_id, directory_key);
CREATE INDEX objects_directory_files_idx ON storage.objects (directory_scope, name);
COMMENT ON COLUMN storage.objects.directory_key IS
    'Derived parent path for indexed directory listing; visibility always comes from objects RLS.';
COMMENT ON COLUMN storage.objects.directory_scope IS
    'Derived listing fingerprint; queries also verify the exact bucket and parent path for collisions.';
