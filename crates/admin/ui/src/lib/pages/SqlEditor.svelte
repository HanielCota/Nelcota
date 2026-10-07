<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import Play from '@lucide/svelte/icons/play'
  import ChevronDown from '@lucide/svelte/icons/chevron-down'
  import { toast } from 'svelte-sonner'
  import CodeEditor from '$lib/components/app/CodeEditor.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte'
  import SaveQueryDialog from '$lib/components/app/SaveQueryDialog.svelte'
  import SqlSidebar from '$lib/components/app/SqlSidebar.svelte'
  import { api } from '$lib/api'
  import { downloadText, toCsv, toJson } from '$lib/download'
  import { SQL_SNIPPETS } from '$lib/sql-snippets'
  import { sqlStore, type SavedQuery } from '$lib/sql-store.svelte'
  import type { SqlResponse, SqlResult } from '$lib/types'

  let schema = $state<Record<string, string[]>>({})
  let defaultSchema = $state('public')
  let running = $state(false)
  let response = $state<SqlResponse | null>(null)
  let elapsed = $state(0)

  // Diálogo de nome: salvar como nova, ou renomear uma existente.
  let dialog = $state<{ mode: 'save' | 'rename'; query?: SavedQuery } | null>(null)
  let dialogOpen = $state(false)
  let toDelete = $state<SavedQuery | null>(null)
  let deleteOpen = $state(false)

  onMount(async () => {
    try {
      const s = await api.get<{ schema: string; tables: Record<string, string[]> }>('/schema')
      schema = s.tables
      defaultSchema = s.schema
    } catch {
      // Sem autocomplete de tabelas; o editor continua funcionando.
    }
  })

  async function run() {
    const code = sqlStore.draft
    if (running || !code.trim()) return
    running = true
    const started = performance.now()
    try {
      response = await api.post<SqlResponse>('/sql', { sql: code })
      sqlStore.remember(code)
    } catch (e) {
      response = { error: { message: (e as Error).message } }
    } finally {
      elapsed = Math.round(performance.now() - started)
      running = false
    }
  }

  /** Salva a consulta aberta; sem nenhuma aberta, pede um nome. */
  function save() {
    if (!sqlStore.draft.trim()) return
    const current = sqlStore.current
    if (current) {
      sqlStore.save(current.name)
      toast.success(`"${current.name}" salva`)
    } else {
      openDialog({ mode: 'save' })
    }
  }

  function openDialog(next: { mode: 'save' | 'rename'; query?: SavedQuery }) {
    dialog = next
    dialogOpen = true
  }

  function submitName(name: string) {
    if (dialog?.mode === 'rename' && dialog.query) {
      sqlStore.rename(dialog.query.id, name)
    } else {
      sqlStore.save(name, true)
      toast.success(`"${name}" salva`)
    }
  }

  function askDelete(query: SavedQuery) {
    toDelete = query
    deleteOpen = true
  }

  function exportResult(result: SqlResult, index: number, format: 'csv' | 'json') {
    const base = (sqlStore.current?.name ?? 'resultado').replace(/[^\w-]+/g, '_')
    const name = `${base}${index > 0 ? `-${index + 1}` : ''}.${format}`
    if (format === 'csv') downloadText(name, toCsv(result.columns, result.rows), 'text/csv;charset=utf-8')
    else downloadText(name, toJson(result.columns, result.rows), 'application/json')
  }

  function onKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
      event.preventDefault()
      save()
    }
  }

  const firstLine = (s: string) => s.trim().split('\n')[0].slice(0, 70)

  const mod = /Mac|iPhone|iPad/.test(navigator.platform) ? '⌘' : 'Ctrl'
</script>

<svelte:window onkeydown={onKeydown} />

<div class="flex h-full min-h-0">
  <SqlSidebar onrename={(query) => openDialog({ mode: 'rename', query })} ondelete={askDelete} />

  <div class="flex min-w-0 flex-1 flex-col">
    <div class="flex min-h-14 shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b px-4 py-2.5">
      <div class="flex min-w-0 items-center gap-2">
        <h1 class="truncate text-sm font-semibold">
          {sqlStore.current?.name ?? 'Nova consulta'}
        </h1>
        {#if sqlStore.dirty}
          <span class="shrink-0 text-xs text-muted-foreground" title="Alterações não salvas">(não salva)</span>
        {/if}
      </div>
      <span
        class="hidden text-xs text-muted-foreground md:inline"
        title="As consultas rodam como dono do banco: o RLS não se aplica."
        >Roda como dono do banco, sem RLS</span
      >
      <div class="ml-auto flex flex-wrap items-center gap-2">
        <!-- Em telas sem a barra lateral, modelos e salvas ficam num menu. -->
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button variant="ghost" size="sm" class="lg:hidden" {...props}>Abrir</Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="w-64">
            {#if sqlStore.saved.length}
              <DropdownMenu.Label class="text-xs text-muted-foreground">Salvas</DropdownMenu.Label>
              {#each sqlStore.sorted as query (query.id)}
                <DropdownMenu.Item onclick={() => sqlStore.openSaved(query.id)}>{query.name}</DropdownMenu.Item>
              {/each}
              <DropdownMenu.Separator />
            {/if}
            <DropdownMenu.Label class="text-xs text-muted-foreground">Modelos</DropdownMenu.Label>
            {#each SQL_SNIPPETS as snippet (snippet.label)}
              <DropdownMenu.Item onclick={() => sqlStore.open(snippet.sql)}>{snippet.label}</DropdownMenu.Item>
            {/each}
          </DropdownMenu.Content>
        </DropdownMenu.Root>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button variant="ghost" size="sm" disabled={sqlStore.history.length === 0} {...props}>Histórico</Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="w-96">
            <DropdownMenu.Label class="text-xs text-muted-foreground">Executadas recentemente</DropdownMenu.Label>
            {#each sqlStore.history as item, i (i)}
              <DropdownMenu.Item class="font-mono text-xs" onclick={() => sqlStore.open(item)}
                ><span class="truncate">{firstLine(item)}</span></DropdownMenu.Item
              >
            {/each}
          </DropdownMenu.Content>
        </DropdownMenu.Root>

        <div class="flex">
          <Button variant="outline" size="sm" class="rounded-r-none" onclick={save} title={`${mod}+S`}>
            Salvar
          </Button>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger>
              {#snippet child({ props })}
                <Button variant="outline" size="icon-sm" class="rounded-l-none border-l-0" aria-label="Mais opções de salvar" {...props}>
                  <ChevronDown />
                </Button>
              {/snippet}
            </DropdownMenu.Trigger>
            <DropdownMenu.Content align="end" class="w-52">
              <DropdownMenu.Item disabled={!sqlStore.draft.trim()} onclick={() => openDialog({ mode: 'save' })}>
                Salvar como nova…
              </DropdownMenu.Item>
              {#if sqlStore.current}
                <DropdownMenu.Item onclick={() => openDialog({ mode: 'rename', query: sqlStore.current! })}>
                  Renomear…
                </DropdownMenu.Item>
              {/if}
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </div>

        <Button onclick={run} disabled={running} title={`Executar (${mod}+Enter)`} class="min-w-28">
          <Play />{running ? 'Executando…' : 'Executar'}
        </Button>
      </div>
    </div>

    <div class="h-[42%] min-h-44 shrink-0 border-b">
      <CodeEditor
        bind:value={() => sqlStore.draft, (sql) => sqlStore.setDraft(sql)}
        {schema}
        {defaultSchema}
        onrun={run}
      />
    </div>

    <div class="relative min-h-0 flex-1 overflow-auto bg-background" aria-busy={running}>
      {#if running}
        <div class="pointer-events-none sticky top-0 z-20 h-0.5 overflow-hidden bg-brand/15" aria-hidden="true">
          <div class="animate-progress h-full w-2/5 bg-brand"></div>
        </div>
      {/if}
      {#if !response}
        <EmptyState
          title="Nenhum resultado ainda"
          description="Escreva uma consulta e execute. Limite de 30 s e 1000 linhas por resultado."
        />
      {:else if response.error}
        <div class="p-4" role="alert">
          <div class="rounded-md border border-destructive/30 px-4 py-3 text-sm">
            <p class="font-mono text-xs leading-relaxed text-destructive">
              {#if response.error.code}{response.error.code}: {/if}{response.error.message}
            </p>
            {#if response.error.position}<p class="mt-2 text-muted-foreground">Posição {response.error.position} no texto.</p>{/if}
            {#if response.error.detail}<p class="mt-2 text-muted-foreground">{response.error.detail}</p>{/if}
            {#if response.error.hint}<p class="mt-2 text-muted-foreground">Dica: {response.error.hint}</p>{/if}
          </div>
        </div>
      {:else if response.results}
        {#if response.results.length === 0}
          <p class="border-b px-4 py-2.5 text-sm text-muted-foreground">
            Executado, sem linhas · <span class="tabular-nums">{elapsed} ms</span>
          </p>
        {/if}
        {#each response.results as result, r (r)}
          <div class="flex min-h-10 items-center gap-2 border-b px-4 py-1 text-sm text-muted-foreground">
            <span class="tabular-nums">
              {#if response.results.length > 1}Resultado {r + 1} de {response.results.length} · {/if}{result.count}
              {result.count === 1 ? 'linha' : 'linhas'}{#if result.truncated}<span class="text-warning">
                  (mostrando 1000)</span
                >{/if} · {elapsed} ms
            </span>
            {#if result.columns.length}
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  {#snippet child({ props })}
                    <Button variant="ghost" size="sm" class="ml-auto" {...props}>Exportar</Button>
                  {/snippet}
                </DropdownMenu.Trigger>
                <DropdownMenu.Content align="end" class="w-40">
                  <DropdownMenu.Item onclick={() => exportResult(result, r, 'csv')}>CSV</DropdownMenu.Item>
                  <DropdownMenu.Item onclick={() => exportResult(result, r, 'json')}>JSON</DropdownMenu.Item>
                </DropdownMenu.Content>
              </DropdownMenu.Root>
            {/if}
          </div>
          {#if result.columns.length}
            <table class="w-max min-w-full border-separate border-spacing-0 text-sm">
              <thead class="sticky top-0 z-10">
                <tr>
                  {#each result.columns as column, c (c)}
                    <th class="border-r border-b bg-muted px-3 py-2 text-left font-mono text-xs font-medium">{column}</th>
                  {/each}
                </tr>
              </thead>
              <tbody>
                {#each result.rows as row, i (i)}
                  <tr class="hover:bg-muted/40">
                    {#each row as cell, c (c)}
                      <td class="max-w-96 truncate border-r border-b px-3 py-2 font-mono text-xs" title={cell ?? 'NULL'}>
                        {#if cell === null}<span class="text-muted-foreground italic">NULL</span>{:else}{cell}{/if}
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
</div>

<SaveQueryDialog
  bind:open={dialogOpen}
  title={dialog?.mode === 'rename' ? 'Renomear consulta' : 'Salvar consulta'}
  initialName={dialog?.mode === 'rename' ? (dialog.query?.name ?? '') : ''}
  confirmLabel={dialog?.mode === 'rename' ? 'Renomear' : 'Salvar'}
  onsubmit={submitName}
/>

{#if toDelete}
  <ConfirmDialog
    bind:open={deleteOpen}
    title={`Apagar "${toDelete.name}"?`}
    description="A consulta salva some deste navegador. Não dá para desfazer."
    confirmLabel="Apagar"
    destructive
    onconfirm={() => sqlStore.remove(toDelete!.id)}
  />
{/if}
