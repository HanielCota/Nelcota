-- Exemplo: tabela com RLS em que cada usuário só enxerga as próprias linhas.
--
-- Não faz parte das migrações do Nelcota (não queremos criar tabelas no banco
-- de ninguém). Os testes de integração aplicam este arquivo; em dev, rode:
--   psql "$NELCOTA_DATABASE_URL" -f examples/todos.sql

CREATE TABLE IF NOT EXISTS public.todos (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id    uuid        NOT NULL DEFAULT auth.uid(),
    title      text        NOT NULL,
    done       boolean     NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE public.todos ENABLE ROW LEVEL SECURITY;

-- anon não recebe GRANT: sem login, nem chega a ver a tabela.
GRANT SELECT, INSERT, UPDATE, DELETE ON public.todos TO authenticated, service_role;

DROP POLICY IF EXISTS todos_dono_select ON public.todos;
CREATE POLICY todos_dono_select ON public.todos
    FOR SELECT TO authenticated
    USING (user_id = auth.uid());

DROP POLICY IF EXISTS todos_dono_insert ON public.todos;
CREATE POLICY todos_dono_insert ON public.todos
    FOR INSERT TO authenticated
    WITH CHECK (user_id = auth.uid());

DROP POLICY IF EXISTS todos_dono_update ON public.todos;
CREATE POLICY todos_dono_update ON public.todos
    FOR UPDATE TO authenticated
    USING (user_id = auth.uid())
    WITH CHECK (user_id = auth.uid());

DROP POLICY IF EXISTS todos_dono_delete ON public.todos;
CREATE POLICY todos_dono_delete ON public.todos
    FOR DELETE TO authenticated
    USING (user_id = auth.uid());
