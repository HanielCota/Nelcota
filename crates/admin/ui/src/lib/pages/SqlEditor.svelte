<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import Play from '@lucide/svelte/icons/play'
  import ChevronDown from '@lucide/svelte/icons/chevron-down'
  import FolderOpen from '@lucide/svelte/icons/folder-open'
  import History from '@lucide/svelte/icons/history'
  import Save from '@lucide/svelte/icons/save'
  import Download from '@lucide/svelte/icons/download'
  import Pencil from '@lucide/svelte/icons/pencil'
  import FileCode from '@lucide/svelte/icons/file-code'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import Maximize2 from '@lucide/svelte/icons/maximize-2'
  import * as Resizable from '$lib/components/ui/resizable'
  import { toast } from 'svelte-sonner'
  import CodeEditor from '$lib/components/app/CodeEditor.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte'
  import SaveQueryDialog from '$lib/components/app/SaveQueryDialog.svelte'
  import SqlSidebar from '$lib/components/app/SqlSidebar.svelte'
  import CellDetailDialog from '$lib/components/app/CellDetailDialog.svelte'
  import { api } from '$lib/api'
  import { downloadText, toCsv, toJson } from '$lib/download'
  import { errorMessage, t } from '$lib/i18n/index.svelte'
  import { SQL_SNIPPETS } from '$lib/sql-snippets'
  import { sqlStore, type SavedQuery, type SqlDraft } from '$lib/sql-store.svelte'
  import type { SqlResponse, SqlResult } from '$lib/types'

  let schema = $state<Record<string, string[]>>({})
  let defaultSchema = $state('public')
  let running = $state(false)
  let response = $state<SqlResponse | null>(null)
  let elapsed = $state(0)
  let execution: AbortController | undefined
  onDestroy(() => execution?.abort())

  // Name dialog: save as a new query, or rename an existing one.
  let dialog = $state<{ mode: 'save' | 'rename'; query?: SavedQuery } | null>(null)
  let dialogOpen = $state(false)
  let toDelete = $state<SavedQuery | null>(null)
  let deleteOpen = $state(false)
  let toDeleteDraft = $state<SqlDraft | null>(null)
  let deleteDraftOpen = $state(false)
  let detail = $state<{ title: string; value: string | null } | null>(null)
  let detailOpen = $state(false)

  onMount(async () => {
    try {
      const s = await api.get<{ schema: string; tables: Record<string, string[]> }>('/schema')
      schema = s.tables
      defaultSchema = s.schema
    } catch {
      // No table autocomplete; the editor keeps working.
    }
  })

  async function run() {
    const code = sqlStore.draft
    if (running || !code.trim()) return
    running = true
    const started = performance.now()
    const controller = execution = new AbortController()
    try {
      response = await api.post<SqlResponse>('/sql', { sql: code }, { signal: controller.signal })
      sqlStore.remember(code)
    } catch (e) {
      response = { error: { message: errorMessage(e), code: null, detail: null, hint: null, position: null } }
    } finally {
      elapsed = Math.round(performance.now() - started)
      running = false
    }
  }

  /** Saves the open query; with none open, asks for a name. */
  function save() {
    if (!sqlStore.draft.trim()) return
    const current = sqlStore.current
    if (current) {
      sqlStore.save(current.name)
      toast.success(t('sql.editor.savedToast', { name: current.name }))
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
      toast.success(t('sql.editor.savedToast', { name }))
    }
  }

  function askDelete(query: SavedQuery) {
    toDelete = query
    deleteOpen = true
  }

  function exportResult(result: SqlResult, index: number, format: 'csv' | 'json') {
    const base = (sqlStore.current?.name ?? t('sql.editor.exportBaseName')).replace(/[^\w-]+/g, '_')
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
  <SqlSidebar onrename={(query) => openDialog({ mode: 'rename', query })} ondelete={askDelete} ondeleteDraft={(draft) => { toDeleteDraft = draft; deleteDraftOpen = true }} />

  <div class="flex min-w-0 flex-1 flex-col">
    <div class="flex min-h-14 shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b px-4 py-2.5">
      <div class="flex min-w-0 items-center gap-2">
        <h1 class="truncate text-sm font-semibold">
          {sqlStore.current?.name ?? t('sql.editor.newQuery')}
        </h1>
        {#if sqlStore.dirty}
          <span class="shrink-0 text-xs text-muted-foreground" title={t('sql.editor.unsavedTitle')}>{t('sql.editor.unsaved')}</span>
        {/if}
      </div>
      <span
        class="hidden text-xs text-muted-foreground md:inline"
        title={t('sql.editor.ownerNoteTitle')}
        >{t('sql.editor.ownerNote')}</span
      >
      <div class="ml-auto flex flex-wrap items-center gap-2">
        <!-- On screens without the sidebar, templates and saved queries live in a menu. -->
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button variant="ghost" size="sm" class="lg:hidden" {...props}><FolderOpen data-icon="inline-start" aria-hidden="true" />{t('sql.editor.open')}</Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="w-64">
            {#if sqlStore.saved.length}
              <DropdownMenu.Label class="text-xs text-muted-foreground">{t('sql.editor.saved')}</DropdownMenu.Label>
              {#each sqlStore.sorted as query (query.id)}
                <DropdownMenu.Item onclick={() => sqlStore.openSaved(query.id)}><FileCode aria-hidden="true" />{query.name}</DropdownMenu.Item>
              {/each}
              <DropdownMenu.Separator />
            {/if}
            {#if sqlStore.looseDrafts.length}
              <DropdownMenu.Label class="text-xs text-muted-foreground">{t('sql.editor.drafts')}</DropdownMenu.Label>
              {#each sqlStore.looseDrafts as draft (draft.id)}
                <DropdownMenu.Item onclick={() => sqlStore.openDraft(draft.id)}><FileCode aria-hidden="true" /><span class="truncate">{firstLine(draft.sql)}</span></DropdownMenu.Item>
              {/each}
              <DropdownMenu.Separator />
            {/if}
            <DropdownMenu.Item onclick={() => sqlStore.open('')}><FileCode aria-hidden="true" />{t('sql.editor.newQuery')}</DropdownMenu.Item>
            <DropdownMenu.Label class="text-xs text-muted-foreground">{t('sql.editor.templates')}</DropdownMenu.Label>
            {#each SQL_SNIPPETS as snippet (snippet.label)}
              <DropdownMenu.Item onclick={() => sqlStore.open(t(snippet.sql))}><FileCode aria-hidden="true" />{t(snippet.label)}</DropdownMenu.Item>
            {/each}
          </DropdownMenu.Content>
        </DropdownMenu.Root>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button variant="ghost" size="sm" disabled={sqlStore.history.length === 0} {...props}><History data-icon="inline-start" aria-hidden="true" />{t('sql.editor.history')}</Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="w-96">
            <DropdownMenu.Label class="text-xs text-muted-foreground">{t('sql.editor.recentlyRun')}</DropdownMenu.Label>
            {#each sqlStore.history as item, i (i)}
              <DropdownMenu.Item class="font-mono text-xs" onclick={() => sqlStore.open(item)}
                ><span class="truncate">{firstLine(item)}</span></DropdownMenu.Item
              >
            {/each}
          </DropdownMenu.Content>
        </DropdownMenu.Root>

        <div class="flex">
          <Button variant="outline" size="sm" class="rounded-r-none" onclick={save} title={`${mod}+S`}>
            <Save data-icon="inline-start" aria-hidden="true" />
            {t('common.save')}
          </Button>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger>
              {#snippet child({ props })}
                <Button variant="outline" size="icon-sm" class="rounded-l-none border-l-0" aria-label={t('sql.editor.saveMore')} {...props}>
                  <ChevronDown />
                </Button>
              {/snippet}
            </DropdownMenu.Trigger>
            <DropdownMenu.Content align="end" class="w-52">
              <DropdownMenu.Item disabled={!sqlStore.draft.trim()} onclick={() => openDialog({ mode: 'save' })}>
                <Save aria-hidden="true" />
                {t('sql.editor.saveAsNew')}
              </DropdownMenu.Item>
              {#if sqlStore.current}
                <DropdownMenu.Item onclick={() => openDialog({ mode: 'rename', query: sqlStore.current! })}>
                  <Pencil aria-hidden="true" />
                  {t('sql.editor.rename')}
                </DropdownMenu.Item>
              {/if}
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </div>

        <Button onclick={run} disabled={running} title={t('sql.editor.runTitle', { shortcut: `${mod}+Enter` })} class="min-w-28">
          {#if running}<LoaderCircle data-icon="inline-start" class="animate-spin" aria-hidden="true" />{:else}<Play data-icon="inline-start" aria-hidden="true" />{/if}{running ? t('sql.editor.running') : t('sql.editor.run')}
        </Button>
      </div>
    </div>

    <p class="border-b px-4 py-1.5 text-xs text-muted-foreground lg:hidden">{t('sql.editor.localOnly')}</p>
    <Resizable.PaneGroup direction="vertical" class="min-h-0 flex-1" autoSaveId="nelcota-sql-layout">
    <Resizable.Pane defaultSize={42} minSize={20}>
    <div class="h-full min-h-0">
      <CodeEditor
        bind:value={() => sqlStore.draft, (sql) => sqlStore.setDraft(sql)}
        {schema}
        {defaultSchema}
        onrun={run}
      />
    </div>
    </Resizable.Pane>
    <Resizable.Handle withHandle aria-label={t('sql.editor.resize')} />
    <Resizable.Pane defaultSize={58} minSize={20}>

    <div class="relative h-full min-h-0 overflow-auto bg-background" aria-busy={running}>
      {#if running}
        <div class="pointer-events-none sticky top-0 z-20 h-0.5 overflow-hidden bg-brand/15" aria-hidden="true">
          <div class="animate-progress h-full w-2/5 bg-brand"></div>
        </div>
      {/if}
      {#if !response}
        <EmptyState
          title={t('sql.results.emptyTitle')}
          icon={FileCode}
          description={t('sql.results.emptyDescription')}
        />
      {:else if 'error' in response}
        <div class="p-4" role="alert">
          <div class="rounded-md border border-destructive/30 px-4 py-3 text-sm">
            <p class="font-mono text-xs leading-relaxed text-destructive">
              {#if response.error.code}{response.error.code}: {/if}{response.error.message}
            </p>
            {#if response.error.position}<p class="mt-2 text-muted-foreground">{t('sql.results.position', { position: response.error.position })}</p>{/if}
            {#if response.error.detail}<p class="mt-2 text-muted-foreground">{response.error.detail}</p>{/if}
            {#if response.error.hint}<p class="mt-2 text-muted-foreground">{t('sql.results.hint', { hint: response.error.hint })}</p>{/if}
          </div>
        </div>
      {:else if 'results' in response}
        {#if response.results_truncated}
          <p class="border-b px-4 py-2.5 text-sm text-warning" role="status">{t('sql.results.batchTruncated')}</p>
        {/if}
        {#if response.results.length === 0}
          <p class="border-b px-4 py-2.5 text-sm text-muted-foreground">
            {t('sql.results.noRows')} · <span class="tabular-nums">{elapsed} ms</span>
          </p>
        {/if}
        {#each response.results as result, r (r)}
          <div class="flex min-h-10 items-center gap-2 border-b px-4 py-1 text-sm text-muted-foreground">
            <span class="tabular-nums">
              {#if response.results.length > 1}{t('sql.results.resultOf', { index: r + 1, total: response.results.length })}
                · {/if}{t('sql.results.rows', { count: result.count })}{#if result.truncated}<span class="text-warning">
                  {t('sql.results.truncated')}</span
                >{/if} · {elapsed} ms
            </span>
            {#if result.columns.length}
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  {#snippet child({ props })}
                    <Button variant="ghost" size="sm" class="ml-auto" {...props}><Download data-icon="inline-start" aria-hidden="true" />{t('common.export')}</Button>
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
                        <div class="flex items-center gap-2"><span class="min-w-0 flex-1 truncate">{#if cell === null}<span class="text-muted-foreground italic">NULL</span>{:else}{cell}{/if}</span>{#if cell && (cell.length > 80 || cell.includes('\n'))}<Button variant="ghost" size="icon-xs" aria-label={t('common.details')} onclick={() => { detail = { title: result.columns[c], value: cell }; detailOpen = true }}><Maximize2 aria-hidden="true" /></Button>{/if}</div>
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
    </Resizable.Pane>
    </Resizable.PaneGroup>
  </div>
</div>

<SaveQueryDialog
  bind:open={dialogOpen}
  title={dialog?.mode === 'rename' ? t('sql.dialog.renameTitle') : t('sql.dialog.saveTitle')}
  initialName={dialog?.mode === 'rename' ? (dialog.query?.name ?? '') : ''}
  confirmLabel={dialog?.mode === 'rename' ? t('sql.dialog.renameConfirm') : t('common.save')}
  onsubmit={submitName}
/>

{#if toDelete}
  <ConfirmDialog
    bind:open={deleteOpen}
    title={t('sql.dialog.deleteTitle', { name: toDelete.name })}
    description={t('sql.dialog.deleteDescription')}
    confirmLabel={t('common.delete')}
    destructive
    onconfirm={() => sqlStore.remove(toDelete!.id)}
  />
{/if}

{#if toDeleteDraft}
  <ConfirmDialog bind:open={deleteDraftOpen} title={t('common.unsavedTitle')} description={t('sql.editor.discardDraft')} confirmLabel={t('common.discard')} destructive onconfirm={() => sqlStore.removeDraft(toDeleteDraft!.id)} />
{/if}
{#if detail}<CellDetailDialog bind:open={detailOpen} title={detail.title} value={detail.value} />{/if}
