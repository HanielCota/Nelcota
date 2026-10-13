-- Nelcota: recarga automática do catálogo da API após DDL.
--
-- Um event trigger manda `NOTIFY nelcota, 'reload schema'` ao fim de cada
-- comando DDL (CREATE/ALTER/DROP, GRANT/REVOKE, policies...). O servidor escuta
-- o canal e recarrega a introspecção. Para forçar a recarga à mão:
--   NOTIFY nelcota, 'reload schema';
--
-- Event triggers exigem superusuário. Sem ele (alguns Postgres gerenciados), a
-- migração segue sem o trigger e a recarga precisa ser manual.

CREATE OR REPLACE FUNCTION nelcota.notificar_ddl() RETURNS event_trigger
    LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM pg_notify('nelcota', 'reload schema');
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_event_trigger WHERE evtname = 'nelcota_ddl') THEN
        CREATE EVENT TRIGGER nelcota_ddl ON ddl_command_end
            EXECUTE FUNCTION nelcota.notificar_ddl();
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_event_trigger WHERE evtname = 'nelcota_drop') THEN
        CREATE EVENT TRIGGER nelcota_drop ON sql_drop
            EXECUTE FUNCTION nelcota.notificar_ddl();
    END IF;
EXCEPTION WHEN insufficient_privilege THEN
    RAISE NOTICE 'sem superusuário: recarga automática do catálogo desabilitada (use NOTIFY nelcota)';
END
$$;
