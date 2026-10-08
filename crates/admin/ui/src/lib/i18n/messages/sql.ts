// SQL editor: toolbar, results, saved queries and ready-made templates.
import type { Messages } from '../index.svelte'

const ptBR = {
  editor: {
    newQuery: 'Nova consulta',
    unsaved: '(não salva)',
    unsavedTitle: 'Alterações não salvas',
    ownerNote: 'Roda como dono do banco, sem RLS',
    ownerNoteTitle: 'As consultas rodam como dono do banco: o RLS não se aplica.',
    open: 'Abrir',
    drafts: 'Rascunhos',
    localOnly: 'Consultas e rascunhos ficam neste navegador, para este projeto.',
    resize: 'Redimensionar editor e resultados',
    discardDraft: 'Este rascunho será removido deste navegador. Não dá para desfazer.',
    saved: 'Salvas',
    templates: 'Modelos',
    history: 'Histórico',
    recentlyRun: 'Executadas recentemente',
    saveMore: 'Mais opções de salvar',
    saveAsNew: 'Salvar como nova…',
    rename: 'Renomear…',
    run: 'Executar',
    runTitle: 'Executar ({shortcut})',
    running: 'Executando…',
    savedToast: '"{name}" salva',
    exportBaseName: 'resultado',
  },
  results: {
    emptyTitle: 'Nenhum resultado ainda',
    emptyDescription: 'Escreva uma consulta e execute. Limite de 30 s e 1000 linhas por resultado.',
    position: 'Posição {position} no texto.',
    hint: 'Dica: {hint}',
    noRows: 'Executado, sem linhas',
    resultOf: 'Resultado {index} de {total}',
    rows: { one: '{count} linha', other: '{count} linhas' },
    truncated: '(resultado limitado por linhas ou tamanho)',
    batchTruncated: 'Exibindo os primeiros 32 resultados. Todos os comandos do lote foram processados.',
  },
  dialog: {
    saveTitle: 'Salvar consulta',
    renameTitle: 'Renomear consulta',
    renameConfirm: 'Renomear',
    description: 'Fica salva neste navegador, para este projeto.',
    placeholder: 'ex.: pedidos da semana',
    deleteTitle: 'Apagar "{name}"?',
    deleteDescription: 'A consulta salva some deste navegador. Não dá para desfazer.',
  },
  sidebar: {
    label: 'Consultas salvas e modelos',
    title: 'Editor SQL',
    savedCount: 'Salvas ({count})',
    noSaved: 'Nenhuma consulta salva.',
    actionsFor: 'Ações de {name}',
    rename: 'Renomear',
  },
  snippets: {
    rlsTable: {
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
    tablesWithoutRls: {
      label: 'Listar tabelas sem RLS',
      sql: `select c.relname as tabela, c.relrowsecurity as rls
from pg_class c
join pg_namespace n on n.oid = c.relnamespace
where n.nspname = 'public' and c.relkind = 'r'
order by c.relrowsecurity, c.relname;`,
    },
    recentUsers: {
      label: 'Usuários recentes',
      sql: `select id, email, created_at, last_sign_in_at
from auth.users
order by created_at desc
limit 20;`,
    },
    slowQueries: {
      label: 'Queries mais lentas (pg_stat_statements)',
      sql: `select round(mean_exec_time::numeric, 2) as ms_medio, calls, query
from extensions.pg_stat_statements
order by mean_exec_time desc
limit 20;`,
    },
  },
}

const en: Messages<typeof ptBR> = {
  editor: {
    newQuery: 'New query',
    unsaved: '(unsaved)',
    unsavedTitle: 'Unsaved changes',
    ownerNote: 'Runs as the database owner, without RLS',
    ownerNoteTitle: 'Queries run as the database owner: RLS does not apply.',
    open: 'Open',
    drafts: 'Drafts',
    localOnly: 'Queries and drafts are stored in this browser, for this project.',
    resize: 'Resize editor and results',
    discardDraft: 'This draft will be removed from this browser. This cannot be undone.',
    saved: 'Saved',
    templates: 'Templates',
    history: 'History',
    recentlyRun: 'Recently run',
    saveMore: 'More save options',
    saveAsNew: 'Save as new…',
    rename: 'Rename…',
    run: 'Run',
    runTitle: 'Run ({shortcut})',
    running: 'Running…',
    savedToast: '"{name}" saved',
    exportBaseName: 'result',
  },
  results: {
    emptyTitle: 'No results yet',
    emptyDescription: 'Write a query and run it. Limit of 30 s and 1000 rows per result.',
    position: 'Position {position} in the text.',
    hint: 'Hint: {hint}',
    noRows: 'Done, no rows',
    resultOf: 'Result {index} of {total}',
    rows: { one: '{count} row', other: '{count} rows' },
    truncated: '(result limited by rows or size)',
    batchTruncated: 'Showing the first 32 results. All statements in the batch were processed.',
  },
  dialog: {
    saveTitle: 'Save query',
    renameTitle: 'Rename query',
    renameConfirm: 'Rename',
    description: 'Saved in this browser, for this project.',
    placeholder: 'e.g. orders this week',
    deleteTitle: 'Delete "{name}"?',
    deleteDescription: 'The saved query is removed from this browser. This cannot be undone.',
  },
  sidebar: {
    label: 'Saved queries and templates',
    title: 'SQL editor',
    savedCount: 'Saved ({count})',
    noSaved: 'No saved queries.',
    actionsFor: 'Actions for {name}',
    rename: 'Rename',
  },
  snippets: {
    rlsTable: {
      label: 'Create a table with RLS',
      sql: `create table public.notes (
  id bigint generated always as identity primary key,
  owner uuid not null default auth.uid(),
  body text not null,
  created_at timestamptz not null default now()
);

alter table public.notes enable row level security;

create policy "owner reads and changes their own notes"
  on public.notes for all to authenticated
  using (owner = auth.uid()) with check (owner = auth.uid());

grant select, insert, update, delete on public.notes to authenticated;`,
    },
    tablesWithoutRls: {
      label: 'List tables without RLS',
      sql: `select c.relname as table_name, c.relrowsecurity as rls
from pg_class c
join pg_namespace n on n.oid = c.relnamespace
where n.nspname = 'public' and c.relkind = 'r'
order by c.relrowsecurity, c.relname;`,
    },
    recentUsers: {
      label: 'Recent users',
      sql: `select id, email, created_at, last_sign_in_at
from auth.users
order by created_at desc
limit 20;`,
    },
    slowQueries: {
      label: 'Slowest queries (pg_stat_statements)',
      sql: `select round(mean_exec_time::numeric, 2) as mean_ms, calls, query
from extensions.pg_stat_statements
order by mean_exec_time desc
limit 20;`,
    },
  },
}

export default { 'pt-BR': ptBR, en }
