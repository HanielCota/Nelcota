-- Bound aggregate state before json_agg appends the next row. The counter is
-- local to the request transaction and includes intermediate embedded JSON.
-- Keep json (not jsonb): numeric precision and the representation are unchanged.
CREATE FUNCTION extensions.nelcota_check_json(value json, max_bytes bigint)
RETURNS json LANGUAGE plpgsql VOLATILE STRICT
SET search_path = pg_catalog
AS $$
DECLARE
    used bigint := coalesce(nullif(current_setting('nelcota.response_bytes', true), ''), '0')::bigint;
    bytes bigint := octet_length(value::text)::bigint + 2;
BEGIN
    IF bytes > max_bytes OR used > max_bytes - bytes THEN
        RAISE EXCEPTION USING ERRCODE = '54000', MESSAGE = 'NELCOTA_RESPONSE_TOO_LARGE';
    END IF;
    PERFORM set_config('nelcota.response_bytes', (used + bytes)::text, true);
    RETURN value;
END
$$;
REVOKE ALL ON FUNCTION extensions.nelcota_check_json(json, bigint) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION extensions.nelcota_check_json(json, bigint) TO anon, authenticated, service_role;
