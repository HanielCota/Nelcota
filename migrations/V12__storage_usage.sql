-- Serialize migration against metadata writers while initializing the counter.
LOCK TABLE storage.objects IN SHARE ROW EXCLUSIVE MODE;
CREATE TABLE storage.usage (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    bytes bigint NOT NULL CHECK (bytes >= 0)
);
INSERT INTO storage.usage (bytes) SELECT coalesce(sum(size), 0)::bigint FROM storage.objects;
REVOKE ALL ON storage.usage FROM PUBLIC, anon, authenticated, service_role;
GRANT SELECT ON storage.usage TO nelcota_storage;

-- The trigger covers ordinary uploads and direct administrative SQL alike.
-- No caller receives permission to modify the aggregate independently.
CREATE FUNCTION storage.track_usage() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog
AS $$
DECLARE delta bigint;
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        UPDATE storage.usage SET bytes = 0 WHERE singleton;
    ELSE
        -- Transition tables aggregate once per statement, including bulk SQL
        -- and both branches of INSERT ... ON CONFLICT DO UPDATE.
        IF TG_OP = 'INSERT' THEN
            SELECT coalesce(sum(size), 0)::bigint INTO delta FROM new_objects;
        ELSIF TG_OP = 'DELETE' THEN
            SELECT -coalesce(sum(size), 0)::bigint INTO delta FROM old_objects;
        ELSE
            SELECT (SELECT coalesce(sum(size), 0) FROM new_objects)
                 - (SELECT coalesce(sum(size), 0) FROM old_objects) INTO delta;
        END IF;
        IF delta <> 0 THEN
            UPDATE storage.usage SET bytes = bytes + delta WHERE singleton;
        END IF;
    END IF;
    RETURN NULL;
END
$$;
REVOKE ALL ON FUNCTION storage.track_usage() FROM PUBLIC;
CREATE TRIGGER objects_usage_insert AFTER INSERT ON storage.objects
REFERENCING NEW TABLE AS new_objects FOR EACH STATEMENT EXECUTE FUNCTION storage.track_usage();
CREATE TRIGGER objects_usage_update AFTER UPDATE ON storage.objects
REFERENCING OLD TABLE AS old_objects NEW TABLE AS new_objects
FOR EACH STATEMENT EXECUTE FUNCTION storage.track_usage();
CREATE TRIGGER objects_usage_delete AFTER DELETE ON storage.objects
REFERENCING OLD TABLE AS old_objects FOR EACH STATEMENT EXECUTE FUNCTION storage.track_usage();
CREATE TRIGGER objects_usage_truncate AFTER TRUNCATE ON storage.objects
FOR EACH STATEMENT EXECUTE FUNCTION storage.track_usage();
COMMENT ON TABLE storage.usage IS 'Transactional byte total; reconcile with SUM(storage.objects.size) only during maintenance with metadata writes locked.';
