SELECT d.id::text, d.endpoint_id::text, e.name AS endpoint_name,
    (e.enabled AND e.deleted_at IS NULL) AS endpoint_enabled,
    d.event_id::text, d.replay_of::text, d.schema_name, d.table_name, d.event,
    d.url, d.status, d.attempts, d.max_attempts,
    to_char(d.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at,
    CASE WHEN d.status = 'pending' THEN to_char(d.next_attempt_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') END AS next_attempt_at,
    to_char(d.finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS finished_at,
    d.last_status, d.last_error
FROM nelcota.webhook_deliveries d JOIN nelcota.webhook_endpoints e ON e.id = d.endpoint_id
