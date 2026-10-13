-- Events commit with the business row. Only the HTTP worker runs afterward.
DO $$ BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'nelcota_webhooks') THEN
        CREATE ROLE nelcota_webhooks NOLOGIN NOINHERIT;
    END IF;
END $$;

CREATE TABLE nelcota.webhook_endpoints (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    table_oid oid NOT NULL,
    schema_name text NOT NULL,
    table_name text NOT NULL,
    url text NOT NULL CHECK (octet_length(url) <= 2048),
    events text[] NOT NULL CHECK (cardinality(events) BETWEEN 1 AND 3 AND events <@ ARRAY['INSERT','UPDATE','DELETE']),
    enabled boolean NOT NULL DEFAULT true,
    signing_secret text,
    max_attempts integer NOT NULL DEFAULT 6 CHECK (max_attempts BETWEEN 1 AND 10),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    deleted_at timestamptz,
    CHECK (deleted_at IS NOT NULL OR signing_secret IS NOT NULL)
);
CREATE INDEX webhook_endpoints_table ON nelcota.webhook_endpoints (table_oid) WHERE deleted_at IS NULL;

CREATE TABLE nelcota.webhook_deliveries (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    endpoint_id uuid NOT NULL REFERENCES nelcota.webhook_endpoints(id),
    event_id uuid NOT NULL,
    replay_of bigint,
    schema_name text NOT NULL,
    table_name text NOT NULL,
    event text NOT NULL CHECK (event IN ('INSERT','UPDATE','DELETE')),
    url text NOT NULL,
    payload text NOT NULL CHECK (octet_length(payload) <= 262144),
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','processing','succeeded','failed','cancelled')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    max_attempts integer NOT NULL CHECK (max_attempts BETWEEN 1 AND 10),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    next_attempt_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    finished_at timestamptz,
    lease_token uuid,
    locked_until timestamptz,
    last_status integer,
    last_error text
);
CREATE INDEX webhook_deliveries_due ON nelcota.webhook_deliveries (next_attempt_at, id) WHERE status = 'pending';
CREATE INDEX webhook_deliveries_expired ON nelcota.webhook_deliveries (locked_until, id) WHERE status = 'processing';
CREATE INDEX webhook_deliveries_endpoint ON nelcota.webhook_deliveries (endpoint_id, id DESC);
CREATE INDEX webhook_deliveries_status ON nelcota.webhook_deliveries (status, id DESC);
CREATE INDEX webhook_deliveries_replay ON nelcota.webhook_deliveries (replay_of) WHERE status IN ('pending','processing') AND replay_of IS NOT NULL;
CREATE INDEX webhook_deliveries_retention ON nelcota.webhook_deliveries (finished_at, id) WHERE finished_at IS NOT NULL;

CREATE TABLE nelcota.webhook_attempts (
    delivery_id bigint NOT NULL REFERENCES nelcota.webhook_deliveries(id) ON DELETE CASCADE,
    number integer NOT NULL,
    started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    finished_at timestamptz,
    status_code integer,
    duration_ms bigint,
    error text,
    PRIMARY KEY (delivery_id, number)
);

-- Exact O(1) admission limits; aggregate once per statement, not per row.
CREATE TABLE nelcota.webhook_queue_usage (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    outstanding bigint NOT NULL DEFAULT 0 CHECK (outstanding >= 0),
    retained bigint NOT NULL DEFAULT 0 CHECK (retained >= 0)
);
INSERT INTO nelcota.webhook_queue_usage DEFAULT VALUES;
CREATE FUNCTION nelcota.track_webhook_queue() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE pending_delta bigint := 0; total_delta bigint := 0; pending_count bigint; total_count bigint;
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        UPDATE nelcota.webhook_queue_usage SET outstanding = 0, retained = 0 WHERE singleton;
        RETURN NULL;
    END IF;
    IF TG_OP IN ('INSERT','UPDATE') THEN
        SELECT count(*) FILTER (WHERE status IN ('pending','processing')), count(*)
        INTO pending_delta, total_delta FROM new_deliveries;
    END IF;
    IF TG_OP IN ('DELETE','UPDATE') THEN
        SELECT pending_delta - count(*) FILTER (WHERE status IN ('pending','processing')), total_delta - count(*)
        INTO pending_delta, total_delta FROM old_deliveries;
    END IF;
    IF pending_delta <> 0 OR total_delta <> 0 THEN
        UPDATE nelcota.webhook_queue_usage SET outstanding = outstanding + pending_delta, retained = retained + total_delta
        WHERE singleton RETURNING outstanding, retained INTO pending_count, total_count;
        IF pending_count > 10000 OR total_count > 100000 THEN
            RAISE EXCEPTION 'webhook queue capacity reached; clear delivery history or resume delivery processing'
                USING ERRCODE = '53300';
        END IF;
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER webhook_queue_insert AFTER INSERT ON nelcota.webhook_deliveries
REFERENCING NEW TABLE AS new_deliveries FOR EACH STATEMENT EXECUTE FUNCTION nelcota.track_webhook_queue();
CREATE TRIGGER webhook_queue_update AFTER UPDATE ON nelcota.webhook_deliveries
REFERENCING OLD TABLE AS old_deliveries NEW TABLE AS new_deliveries FOR EACH STATEMENT EXECUTE FUNCTION nelcota.track_webhook_queue();
CREATE TRIGGER webhook_queue_delete AFTER DELETE ON nelcota.webhook_deliveries
REFERENCING OLD TABLE AS old_deliveries FOR EACH STATEMENT EXECUTE FUNCTION nelcota.track_webhook_queue();
CREATE TRIGGER webhook_queue_truncate AFTER TRUNCATE ON nelcota.webhook_deliveries
FOR EACH STATEMENT EXECUTE FUNCTION nelcota.track_webhook_queue();

-- One trigger per watched table, one payload per changed row, one bulk enqueue
-- for all interested destinations. No network call, JWT or secret in a payload.
CREATE FUNCTION nelcota.capture_webhook_event() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE event_id uuid; body text; failure text;
BEGIN
    IF NOT EXISTS (SELECT FROM nelcota.webhook_endpoints
        WHERE table_oid = TG_RELID AND enabled AND deleted_at IS NULL AND TG_OP = ANY(events)) THEN
        RETURN NULL;
    END IF;
    event_id := gen_random_uuid();
    body := jsonb_build_object(
        'version', 1, 'id', event_id, 'schema', TG_TABLE_SCHEMA, 'table', TG_TABLE_NAME,
        'type', TG_OP, 'occurred_at', clock_timestamp(),
        'record', CASE WHEN TG_OP = 'DELETE' THEN NULL ELSE to_jsonb(NEW) END,
        'old_record', CASE WHEN TG_OP = 'INSERT' THEN NULL ELSE to_jsonb(OLD) END
    )::text;
    -- A large business row still commits, but the event is visibly failed.
    -- Never send truncated JSON or keep an unbounded copy of its contents.
    IF octet_length(body) > 262144 THEN
        failure := 'payload_too_large';
        body := jsonb_build_object('version', 1, 'id', event_id, 'schema', TG_TABLE_SCHEMA,
            'table', TG_TABLE_NAME, 'type', TG_OP, 'record', NULL, 'old_record', NULL,
            'payload_omitted', true)::text;
    END IF;
    INSERT INTO nelcota.webhook_deliveries
        (endpoint_id, event_id, schema_name, table_name, event, url, payload, max_attempts, status, last_error, finished_at)
    SELECT id, event_id, TG_TABLE_SCHEMA, TG_TABLE_NAME, TG_OP, url, body, max_attempts,
        CASE WHEN failure IS NULL THEN 'pending' ELSE 'failed' END, failure,
        CASE WHEN failure IS NULL THEN NULL ELSE clock_timestamp() END
    FROM nelcota.webhook_endpoints
    WHERE table_oid = TG_RELID AND enabled AND deleted_at IS NULL AND TG_OP = ANY(events);
    RETURN NULL;
END $$;

REVOKE ALL ON nelcota.webhook_endpoints, nelcota.webhook_deliveries, nelcota.webhook_attempts,
    nelcota.webhook_queue_usage FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION nelcota.capture_webhook_event(), nelcota.track_webhook_queue() FROM PUBLIC;
GRANT USAGE ON SCHEMA nelcota TO nelcota_webhooks;
GRANT nelcota_webhooks TO CURRENT_USER;
GRANT SELECT ON nelcota.webhook_endpoints TO nelcota_webhooks;
GRANT SELECT, UPDATE, DELETE ON nelcota.webhook_deliveries TO nelcota_webhooks;
GRANT SELECT, INSERT, UPDATE ON nelcota.webhook_attempts TO nelcota_webhooks;
COMMENT ON TABLE nelcota.webhook_endpoints IS 'Panel-owned egress destinations. Signing keys are sensitive; keep database backups private.';
COMMENT ON TABLE nelcota.webhook_deliveries IS 'At-least-once deliveries. Terminal history is retained for 7 days. Deduplicate on event_id.';
