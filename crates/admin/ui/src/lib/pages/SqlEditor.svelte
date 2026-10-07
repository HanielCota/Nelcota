<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import Play from '@lucide/svelte/icons/play'
  import Save from '@lucide/svelte/icons/save'
  import ChevronDown from '@lucide/svelte/icons/chevron-down'
  import Download from '@lucide/svelte/icons/download'
  import SquareTerminal from '@lucide/svelte/icons/square-terminal'
  import FileCode from '@lucide/svelte/icons/file-code'
  import FilePlus from '@lucide/svelte/icons/file-plus'
  import FolderOpen from '@lucide/svelte/icons/folder-open'
  import History from '@lucide/svelte/icons/history'
  import Pencil from '@lucide/svelte/icons/pencil'
  import Sparkles from '@lucide/svelte/icons/sparkles'
  import ShieldOff from '@lucide/svelte/icons/shield-off'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import CircleCheck from '@lucide/svelte/icons/circle-check'
  import Timer from '@lucide/svelte/icons/timer'
  import { toast } from 'svelte-sonner'
  import CodeEditor from '$lib/components/app/CodeEditor.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import Callout from '$lib/components/app/Callout.svelte'
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
  const kbd =
    'inline-grid min-w-6 place-items-center rounded-md border border-border-strong bg-card px-1.5 py-0.5 font-sans text-2xs font-medium text-foreground shadow-card'
</script>

<svelte:window onkeydown={onKeydown} />

<div class="flex h-full min-h-0">
  <SqlSidebar onrename={(query) => openDialog({ mode: 'rename', query })} ondelete={askDelete} />

  <div class="flex min-w-0 flex-1 flex-col">
    <div class="flex min-h-14 shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b bg-sidebar px-4 py-2.5">
      <div class="flex min-w-0 items-center gap-2.5">
        <FileCode class="hidden size-[18px] shrink-0 text-muted-foreground sm:block" strokeWidth={1.75} />
        <h1 class="truncate text-base font-semibold">
          {sqlStore.current?.name ?? 'Nova consulta'}
        </h1>
        {#if sqlStore.dirty}
          <span class="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground" title="Alterações não salvas">
            <span class="size-2 rounded-full bg-warning"></span><span class="hidden sm:inline">não salva</span>
          </span>
        {/if}
      </div>
      <span
        class="hidden items-center gap-1.5 rounded-full border border-warning/30 bg-warning/10 px-2.5 py-0.5 text-xs font-semibold text-warning md:inline-flex"
        title="As consultas rodam como dono do banco: o RLS não se aplica."
        ><ShieldOff class="size-3.5" />dono do banco · sem RLS</span
      >
      <div class="ml-auto flex flex-wrap items-center gap-2">
        <!-- Em telas sem a barra lateral, modelos e salvas ficam num menu. -->
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button variant="ghost" size="sm" class="lg:hidden" {...props}><FolderOpen />Abrir</Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="w-64">
            {#if sqlStore.saved.length}
              <DropdownMenu.Label class="text-xs text-muted-foreground">Salvas</DropdownMenu.Label>
              {#each sqlStore.sorted as query (query.id)}
                <DropdownMenu.Item onclick={() => sqlStore.openSaved(query.id)}><FileCode />{query.name}</DropdownMenu.Item>
              {/each}
              <DropdownMenu.Separator />
            {/if}
            <DropdownMenu.Label class="text-xs text-muted-foreground">Modelos</DropdownMenu.Label>
            {#each SQL_SNIPPETS as snippet (snippet.label)}
              <DropdownMenu.Item onclick={() => sqlStore.open(snippet.sql)}><Sparkles />{snippet.label}</DropdownMenu.Item>
            {/each}
          </DropdownMenu.Content>
        </DropdownMenu.Root>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button variant="ghost" size="sm" disabled={sqlStore.history.length === 0} {...props}>
                <History />Histórico
              </Button>
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
            <DropdownMenu.Content align="end" class="w-52">
              <DropdownMenu.Item disabled={!sqlStore.draft.trim()} onclick={() => openDialog({ mode: 'save' })}>
                <FilePlus />Salvar como nova…
              </DropdownMenu.Item>
              {#if sqlStore.current}
                <DropdownMenu.Item onclick={() => openDialog({ mode: 'rename', query: sqlStore.current! })}>
                  <Pencil />Renomear…
                </DropdownMenu.Item>
              {/if}
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </div>

        <Button onclick={run} disabled={running} title={`${mod}+Enter`} class="min-w-32">
          {#if running}
            <LoaderCircle class="animate-spin" />Executando…
          {:else}
            <Play class="fill-current" />Executar
            <kbd
              class="ml-0.5 hidden rounded-md border border-white/25 bg-white/10 px-1.5 py-px font-sans text-2xs font-medium sm:inline"
              >{mod} ↵</kbd
            >
          {/if}
        </Button>
      </div>
    </div>

    <div class="h-[42%] min-h-44 shrink-0 border-b bg-card">
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
        <div class="grid h-full place-items-center p-6">
          <EmptyState icon={SquareTerminal} title="Os resultados aparecem aqui" class="w-full max-w-lg border-0 bg-transparent">
            Escreva uma consulta e execute. Limite de 30 s e 1000 linhas por resultado.
            <span class="mt-4 flex flex-wrap justify-center gap-x-4 gap-y-2 text-xs">
              <span class="flex items-center gap-1.5"><kbd class={kbd}>{mod}</kbd><kbd class={kbd}>↵</kbd>executar</span>
              <span class="flex items-center gap-1.5"><kbd class={kbd}>{mod}</kbd><kbd class={kbd}>S</kbd>salvar</span>
              <span class="flex items-center gap-1.5"><kbd class={kbd}>{mod}</kbd><kbd class={kbd}>K</kbd>buscar</span>
            </span>
          </EmptyState>
        </div>
      {:else if response.error}
        <div class="p-4 sm:p-6">
          <Callout variant="danger" title={response.error.code ? `Erro ${response.error.code}` : 'A consulta falhou'}>
            <p class="font-mono text-xs leading-relaxed text-foreground">{response.error.message}</p>
            {#if response.error.position}<p class="mt-2">Posição {response.error.position} no texto.</p>{/if}
            {#if response.error.detail}<p class="mt-2">{response.error.detail}</p>{/if}
            {#if response.error.hint}<p class="mt-2"><span class="font-semibold text-foreground">Dica:</span> {response.error.hint}</p>{/if}
          </Callout>
        </div>
      {:else if response.results}
        {#if response.results.length === 0}
          <div class="flex items-center gap-2.5 border-b bg-sidebar px-4 py-2.5 text-sm text-muted-foreground">
            <CircleCheck class="size-4 text-brand" />Executado, sem linhas.
            <span class="ml-auto font-mono text-xs tabular-nums">{elapsed} ms</span>
          </div>
        {/if}
        {#each response.results as result, r (r)}
          <div class="flex min-h-11 items-center gap-3 border-b bg-sidebar px-4 py-1.5 text-sm">
            <CircleCheck class="size-4 shrink-0 text-brand" />
            <span class="font-semibold tabular-nums">
              {result.count}
              {result.count === 1 ? 'linha' : 'linhas'}
            </span>
            {#if result.truncated}
              <span class="rounded-full border border-warning/30 bg-warning/10 px-2 py-px text-2xs font-semibold text-warning"
                >mostrando 1000</span
              >
            {/if}
            {#if response.results.length > 1}
              <span class="text-xs text-muted-foreground">resultado {r + 1} de {response.results.length}</span>
            {/if}
            <span class="flex items-center gap-1 font-mono text-xs text-muted-foreground tabular-nums">
              <Timer class="size-3.5" />{elapsed} ms
            </span>
            {#if result.columns.length}
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  {#snippet child({ props })}
                    <Button variant="ghost" size="sm" class="ml-auto" {...props}>
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
            <table class="w-max min-w-full border-separate border-spacing-0 text-sm">
              <thead class="sticky top-0 z-10">
                <tr>
                  <th class="w-12 border-r border-b bg-muted px-3 py-2.5 text-right text-xs font-medium text-muted-foreground">#</th>
                  {#each result.columns as column, c (c)}
                    <th class="border-r border-b bg-muted px-4 py-2.5 text-left font-mono text-xs font-semibold">{column}</th>
                  {/each}
                </tr>
              </thead>
              <tbody>
                {#each result.rows as row, i (i)}
                  <tr class="transition-colors even:bg-muted/20 hover:bg-muted/50">
                    <td class="border-r border-b px-3 py-2 text-right font-mono text-2xs text-muted-foreground tabular-nums">{i + 1}</td>
                    {#each row as cell, c (c)}
                      <td class="max-w-96 truncate border-r border-b px-4 py-2 font-mono text-xs" title={cell ?? 'NULL'}>
                        {#if cell === null}<span class="rounded bg-muted px-1 text-2xs text-muted-foreground">NULL</span>{:else}{cell}{/if}
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
