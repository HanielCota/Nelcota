SELECT e.id::text, e.name,
    coalesce(n.nspname, e.schema_name) AS schema_name,
    coalesce(c.relname, e.table_name) AS table_name,
    c.oid IS NOT NULL AS table_available, e.url, e.events, e.enabled, e.max_attempts,
    to_char(e.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at
FROM nelcota.webhook_endpoints e
LEFT JOIN pg_class c ON c.oid = e.table_oid
LEFT JOIN pg_namespace n ON n.oid = c.relnamespace
