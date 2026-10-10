-- Example: a table with RLS where each user only sees their own rows.
--
-- Not part of Nelcota's migrations (we do not want to create tables in anyone's
-- database). The integration tests apply this file. In dev, copy it to your
-- project's migrations/ folder (V1__todos.sql) and run `nelcota migrate`, or
-- paste it into the SQL editor of the panel. Safe to run more than once.

CREATE TABLE IF NOT EXISTS public.todos (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id    uuid        NOT NULL DEFAULT auth.uid(),
    title      text        NOT NULL,
    done       boolean     NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE public.todos ENABLE ROW LEVEL SECURITY;

-- anon gets no GRANT: without a login it does not even see the table.
GRANT SELECT, INSERT, UPDATE, DELETE ON public.todos TO authenticated, service_role;

DROP POLICY IF EXISTS todos_owner_select ON public.todos;
CREATE POLICY todos_owner_select ON public.todos
    FOR SELECT TO authenticated
    USING (user_id = auth.uid());

DROP POLICY IF EXISTS todos_owner_insert ON public.todos;
CREATE POLICY todos_owner_insert ON public.todos
    FOR INSERT TO authenticated
    WITH CHECK (user_id = auth.uid());

DROP POLICY IF EXISTS todos_owner_update ON public.todos;
CREATE POLICY todos_owner_update ON public.todos
    FOR UPDATE TO authenticated
    USING (user_id = auth.uid())
    WITH CHECK (user_id = auth.uid());

DROP POLICY IF EXISTS todos_owner_delete ON public.todos;
CREATE POLICY todos_owner_delete ON public.todos
    FOR DELETE TO authenticated
    USING (user_id = auth.uid());
