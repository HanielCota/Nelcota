<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import Play from '@lucide/svelte/icons/play'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import History from '@lucide/svelte/icons/history'
  import FileCode from '@lucide/svelte/icons/file-code-2'
  import SquareTerminal from '@lucide/svelte/icons/square-terminal'
  import CircleX from '@lucide/svelte/icons/circle-x'
  import CircleCheck from '@lucide/svelte/icons/circle-check'
  import CodeEditor from '$lib/components/app/CodeEditor.svelte'
  import { api } from '$lib/api'
  import type { SqlResponse } from '$lib/types'

  const HISTORY_KEY = 'nelcota:sql-history'
  const DRAFT_KEY = 'nelcota:sql-draft'

  const snippets = [
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

  function read(key: string, fallback: string): string {
    try {
      return localStorage.getItem(key) ?? fallback
    } catch {
      return fallback
    }
  }

  let code = $state(read(DRAFT_KEY, 'select now();'))
  let schema = $state<Record<string, string[]>>({})
  let defaultSchema = $state('public')
  let running = $state(false)
  let response = $state<SqlResponse | null>(null)
  let elapsed = $state(0)
  function readHistory(): string[] {
    try {
      const parsed: unknown = JSON.parse(read(HISTORY_KEY, '[]'))
      return Array.isArray(parsed) ? parsed.filter((h) => typeof h === 'string') : []
    } catch {
      return []
    }
  }

  let history = $state<string[]>(readHistory())

  onMount(async () => {
    try {
      const s = await api.get<{ schema: string; tables: Record<string, string[]> }>('/schema')
      schema = s.tables
      defaultSchema = s.schema
    } catch {
      // Sem autocomplete de tabelas; o editor continua funcionando.
    }
  })

  $effect(() => {
    try {
      localStorage.setItem(DRAFT_KEY, code)
    } catch {
      /* armazenamento indisponível */
    }
  })

  async function run() {
    if (running || !code.trim()) return
    running = true
    const started = performance.now()
    try {
      response = await api.post<SqlResponse>('/sql', { sql: code })
      history = [code, ...history.filter((h) => h !== code)].slice(0, 20)
      try {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(history))
      } catch {
        /* armazenamento indisponível */
      }
    } catch (e) {
      response = { error: { message: (e as Error).message } }
    } finally {
      elapsed = Math.round(performance.now() - started)
      running = false
    }
  }

  const firstLine = (s: string) => s.trim().split('\n')[0].slice(0, 70)
</script>

<div class="flex h-full min-h-0 flex-col">
  <div class="flex shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2.5">
    <SquareTerminal class="size-4 text-muted-foreground" />
    <h1 class="font-semibold">Editor SQL</h1>
    <span class="rounded-full border border-warning/30 bg-warning/10 px-2 py-0.5 text-[11px] font-medium text-warning">
      dono do banco · ignora RLS
    </span>
    <div class="ml-auto flex items-center gap-2">
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button variant="outline" size="sm" {...props}><FileCode />Modelos</Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end" class="w-64">
          {#each snippets as snippet (snippet.label)}
            <DropdownMenu.Item onclick={() => (code = snippet.sql)}>{snippet.label}</DropdownMenu.Item>
          {/each}
        </DropdownMenu.Content>
      </DropdownMenu.Root>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button variant="outline" size="sm" disabled={history.length === 0} {...props}><History />Histórico</Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end" class="w-96">
          {#each history as item, i (i)}
            <DropdownMenu.Item class="font-mono text-xs" onclick={() => (code = item)}>{firstLine(item)}</DropdownMenu.Item>
          {/each}
        </DropdownMenu.Content>
      </DropdownMenu.Root>
      <Button size="sm" onclick={run} disabled={running}>
        {#if running}<LoaderCircle class="animate-spin" />{:else}<Play />{/if}
        Executar
        <kbd class="ml-1 rounded bg-primary-foreground/15 px-1 font-mono text-[10px]">Ctrl ↵</kbd>
      </Button>
    </div>
  </div>

  <div class="h-[42%] min-h-40 shrink-0 border-b bg-sidebar/40">
    <CodeEditor bind:value={code} {schema} {defaultSchema} onrun={run} />
  </div>

  <div class="min-h-0 flex-1 overflow-auto">
    {#if !response}
      <div class="grid h-full place-items-center text-sm text-muted-foreground">
        Execute uma consulta para ver o resultado. Timeout de 30 s; até 1000 linhas por resultado.
      </div>
    {:else if response.error}
      <div class="m-4 rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm">
        <p class="flex items-center gap-2 font-medium text-destructive">
          <CircleX class="size-4" />
          {#if response.error.code}<span class="font-mono">{response.error.code}</span> ·{/if}
          {response.error.message}
        </p>
        {#if response.error.position}<p class="mt-2 text-muted-foreground">Posição {response.error.position} no texto.</p>{/if}
        {#if response.error.detail}<p class="mt-2 text-muted-foreground">{response.error.detail}</p>{/if}
        {#if response.error.hint}<p class="mt-2 text-muted-foreground">Dica: {response.error.hint}</p>{/if}
      </div>
    {:else if response.results}
      {#if response.results.length === 0}
        <p class="flex items-center gap-2 px-4 py-3 text-xs text-muted-foreground">
          <CircleCheck class="size-3.5 text-primary" />Executado sem resultado · {elapsed} ms
        </p>
      {/if}
      {#each response.results as result, r (r)}
        <div class="flex items-center gap-2 border-b bg-muted/30 px-4 py-2 text-xs text-muted-foreground">
          <CircleCheck class="size-3.5 text-primary" />
          {result.count} linha(s){result.truncated ? ' · mostrando 1000' : ''} · {elapsed} ms
        </div>
        {#if result.columns.length}
          <table class="w-max min-w-full border-separate border-spacing-0 text-xs">
            <thead class="sticky top-0">
              <tr>
                {#each result.columns as column, c (c)}
                  <th class="border-r border-b bg-muted px-3 py-2 text-left font-medium">{column}</th>
                {/each}
              </tr>
            </thead>
            <tbody>
              {#each result.rows as row, i (i)}
                <tr class="hover:bg-muted/40">
                  {#each row as cell, c (c)}
                    <td class="max-w-96 truncate border-r border-b px-3 py-1.5 font-mono" title={cell ?? 'NULL'}>
                      {#if cell === null}<span class="text-muted-foreground/70 italic">NULL</span>{:else}{cell}{/if}
                    </td>
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      {/each}
    {/if}
  </div>
</div>
