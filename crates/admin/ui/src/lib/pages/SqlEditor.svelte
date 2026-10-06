<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import Play from '@lucide/svelte/icons/play'
  import Save from '@lucide/svelte/icons/save'
  import ChevronDown from '@lucide/svelte/icons/chevron-down'
  import Download from '@lucide/svelte/icons/download'
  import SquareTerminal from '@lucide/svelte/icons/square-terminal'
  import { toast } from 'svelte-sonner'
  import CodeEditor from '$lib/components/app/CodeEditor.svelte'
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
</script>

<svelte:window onkeydown={onKeydown} />

<div class="flex h-full min-h-0">
  <SqlSidebar onrename={(query) => openDialog({ mode: 'rename', query })} ondelete={askDelete} />

  <div class="flex min-w-0 flex-1 flex-col">
    <div class="flex h-12 shrink-0 flex-wrap items-center gap-3 border-b bg-sidebar px-4">
      <h1 class="truncate text-sm font-medium">
        {sqlStore.current?.name ?? 'Nova consulta'}{#if sqlStore.dirty}<span class="text-muted-foreground"> •</span>{/if}
      </h1>
      <span
        class="hidden rounded-full border border-warning/30 bg-warning/10 px-2 py-px text-[11px] font-medium text-warning sm:inline"
        >dono do banco · sem RLS</span
      >
      <div class="ml-auto flex items-center gap-2">
        <!-- Em telas sem a barra lateral, modelos e salvas ficam num menu. -->
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button variant="ghost" size="sm" class="lg:hidden" {...props}>Abrir</Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="w-64">
            {#if sqlStore.saved.length}
              <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">Salvas</DropdownMenu.Label>
              {#each sqlStore.sorted as query (query.id)}
                <DropdownMenu.Item onclick={() => sqlStore.openSaved(query.id)}>{query.name}</DropdownMenu.Item>
              {/each}
              <DropdownMenu.Separator />
            {/if}
            <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">Modelos</DropdownMenu.Label>
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
            {#each sqlStore.history as item, i (i)}
              <DropdownMenu.Item class="font-mono text-xs" onclick={() => sqlStore.open(item)}
                >{firstLine(item)}</DropdownMenu.Item
              >
            {/each}
          </DropdownMenu.Content>
        </DropdownMenu.Root>

        <div class="flex">
          <Button variant="outline" size="sm" class="rounded-r-none" onclick={save} title="Ctrl+S">
            <Save />Salvar
          </Button>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger>
              {#snippet child({ props })}
                <Button variant="outline" size="icon-sm" class="rounded-l-none border-l-0" aria-label="Mais opções de salvar" {...props}>
                  <ChevronDown />
                </Button>
              {/snippet}
            </DropdownMenu.Trigger>
            <DropdownMenu.Content align="end" class="w-48">
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

        <Button size="sm" onclick={run} disabled={running} title="Ctrl+Enter">
          <Play />{running ? 'Executando…' : 'Executar'}
          <kbd class="ml-1 hidden rounded border border-white/20 px-1 font-sans text-[10px] font-normal opacity-80 sm:inline"
            >Ctrl ↵</kbd
          >
        </Button>
      </div>
    </div>

    <div class="h-[42%] min-h-40 shrink-0 border-b">
      <CodeEditor
        bind:value={() => sqlStore.draft, (sql) => sqlStore.setDraft(sql)}
        {schema}
        {defaultSchema}
        onrun={run}
      />
    </div>

    <div class="min-h-0 flex-1 overflow-auto">
      {#if !response}
        <div class="grid h-full place-items-center p-8 text-center">
          <div>
            <SquareTerminal class="mx-auto size-7 text-muted-foreground" strokeWidth={1.3} />
            <p class="mt-3 text-sm font-medium">Os resultados aparecem aqui</p>
            <p class="mt-1 text-sm font-light text-muted-foreground">
              Ctrl+Enter executa, Ctrl+S salva. Limite de 30 s e 1000 linhas por resultado.
            </p>
          </div>
        </div>
      {:else if response.error}
        <div class="p-4 text-sm">
          <p class="text-destructive">
            {#if response.error.code}<span class="font-mono">{response.error.code}</span>{/if}
            {response.error.message}
          </p>
          {#if response.error.position}<p class="mt-2 text-muted-foreground">Posição {response.error.position} no texto.</p>{/if}
          {#if response.error.detail}<p class="mt-2 text-muted-foreground">{response.error.detail}</p>{/if}
          {#if response.error.hint}<p class="mt-2 text-muted-foreground">Dica: {response.error.hint}</p>{/if}
        </div>
      {:else if response.results}
        {#if response.results.length === 0}
          <p class="px-4 py-3 text-xs text-muted-foreground">Executado, sem linhas. {elapsed} ms</p>
        {/if}
        {#each response.results as result, r (r)}
          <div class="flex items-center gap-2 border-b bg-sidebar px-4 py-1.5 text-xs text-muted-foreground">
            <span class="size-1.5 rounded-full bg-brand"></span>
            {result.count}
            {result.count === 1 ? 'linha' : 'linhas'}{result.truncated ? ' (mostrando 1000)' : ''}, {elapsed} ms
            {#if result.columns.length}
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  {#snippet child({ props })}
                    <Button variant="ghost" size="xs" class="ml-auto text-muted-foreground" {...props}>
                      <Download />Exportar
                    </Button>
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
            <table class="w-max min-w-full border-separate border-spacing-0 text-xs">
              <thead class="sticky top-0">
                <tr>
                  {#each result.columns as column, c (c)}
                    <th class="border-r border-b bg-card px-3 py-2 text-left font-medium">{column}</th>
                  {/each}
                </tr>
              </thead>
              <tbody>
                {#each result.rows as row, i (i)}
                  <tr class="hover:bg-muted/50">
                    {#each row as cell, c (c)}
                      <td class="max-w-96 truncate border-r border-b px-3 py-1.5 font-mono" title={cell ?? 'NULL'}>
                        {#if cell === null}<span class="text-muted-foreground">NULL</span>{:else}{cell}{/if}
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
