// Modelos prontos do editor SQL (também aparecem na paleta de comandos).

export interface SqlSnippet {
  label: string
  sql: string
}

export const SQL_SNIPPETS: readonly SqlSnippet[] = [
  {
    label: 'Criar tabela com RLS',
    sql: `create table public.notas (
  id bigint generated always as identity primary key,
  dono uuid not null default auth.uid(),
  texto text not null,
  criada_em timestamptz not null default now()
);

alter table public.notas enable row level security;

create policy "dono vê e altera as próprias notas"
  on public.notas for all to authenticated
  using (dono = auth.uid()) with check (dono = auth.uid());

grant select, insert, update, delete on public.notas to authenticated;`,
  },
  {
    label: 'Listar tabelas sem RLS',
    sql: `select c.relname as tabela, c.relrowsecurity as rls
from pg_class c
join pg_namespace n on n.oid = c.relnamespace
where n.nspname = 'public' and c.relkind = 'r'
order by c.relrowsecurity, c.relname;`,
  },
  {
    label: 'Usuários recentes',
    sql: `select id, email, created_at, last_sign_in_at
from auth.users
order by created_at desc
limit 20;`,
  },
  {
    label: 'Queries mais lentas (pg_stat_statements)',
    sql: `select round(mean_exec_time::numeric, 2) as ms_medio, calls, query
from extensions.pg_stat_statements
order by mean_exec_time desc
limit 20;`,
  },
]
