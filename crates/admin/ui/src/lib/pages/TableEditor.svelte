<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import Plus from '@lucide/svelte/icons/plus'
  import ShieldAlert from '@lucide/svelte/icons/shield-alert'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import { toast } from 'svelte-sonner'
  import TableSidebar from '$lib/components/app/TableSidebar.svelte'
  import TableToolbar from '$lib/components/app/TableToolbar.svelte'
  import FilterChips from '$lib/components/app/FilterChips.svelte'
  import DataGrid from '$lib/components/app/DataGrid.svelte'
  import GridFooter from '$lib/components/app/GridFooter.svelte'
  import SelectionBar from '$lib/components/app/SelectionBar.svelte'
  import GridState from '$lib/components/app/GridState.svelte'
  import RowSheet from '$lib/components/app/RowSheet.svelte'
  import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte'
  import FilterBar from '$lib/components/app/FilterBar.svelte'
  import CreateTableSheet from '$lib/components/app/CreateTableSheet.svelte'
  import StructureView from '$lib/components/app/StructureView.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import { api, enc } from '$lib/api'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { HiddenColumns } from '$lib/hidden-columns.svelte'
  import { filtersParam, filtersToSearch, parseFilters, type TableFilter } from '$lib/filters'
  import { href, navigate, route } from '$lib/router.svelte'
  import { cn } from '$lib/utils'
  import { rowKey, rowPk } from '$lib/grid'
  import { parseTableView, tableViewSearch, type TableView } from '$lib/table-view'
  import type { Column, RowData, TableData, TablesResponse } from '$lib/types'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  let { name, view = 'data' }: { name?: string; view?: 'data' | 'structure' } = $props()

  const tablesResource = new RemoteResource<TablesResponse>()
  const tables = $derived(tablesResource.data?.tables ?? [])
  const tablesLoading = $derived(tablesResource.loading)
  const tablesError = $derived(tablesResource.error ? errorMessage(tablesResource.error) : '')
  const resource = new RemoteResource<TableData>()
  const data = $derived(resource.data)
  const loading = $derived(resource.loading)
  let savingCell = $state(false)
  const error = $derived(resource.error ? errorMessage(resource.error) : '')
  const tableView = $derived(parseTableView(route.query))
  const page = $derived(tableView.page)
  const size = $derived(tableView.size)
  const sort = $derived(tableView.sort)
  let selected = $state<Set<number>>(new Set())

  let sheetOpen = $state(false)
  let sheetRow = $state<RowData | null>(null)
  let confirmOpen = $state(false)
  let filterOpen = $state(false)
  let filterPreset = $state<string | undefined>(undefined)
  let createOpen = $state(false)

  // Hidden columns, remembered per table.
  const hidden = new HiddenColumns()
  $effect(() => {
    if (name) hidden.load(name)
  })
  $effect(() => {
    if (data) hidden.prune(data.table.columns.map((c) => c.name))
  })

  // Filters come from the URL (`?price=gte.10`): links can be shared and the
  // browser's back button works. The text key avoids reloading without a real change.
  const filters = $derived(parseFilters(route.query))
  const filtersKey = $derived(filtersToSearch(filters))

  async function loadTables() {
    await tablesResource.load(signal => api.get<TablesResponse>('/tables', { signal }))
  }

  onMount(() => {
    loadTables()
    if (route.query.get('create') === 'true') {
      createOpen = true
      navigate('/tables', true)
    }
    return () => tablesResource.cancel()
  })

  async function onCreated(table: string) {
    await loadTables()
    navigate(`/tables/${enc(table)}`)
  }

  async function onRenamed(table: string) {
    await loadTables()
    navigate(`/tables/${enc(table)}/structure`, true)
  }

  async function onDropped() {
    await loadTables()
    navigate('/tables')
  }

  /** Current order and filters, in the API format (listing and export). */
  function rowsParams(): URLSearchParams {
    const params = new URLSearchParams()
    if (sort) {
      params.set('sort', sort.column)
      if (sort.desc) params.set('desc', 'true')
    }
    const f = filtersParam(filters)
    if (f) params.set('filters', f)
    return params
  }

  async function load() {
    if (!name) return
    if (data?.table.name !== name) resource.clear()
    if (view !== 'data') return
    selected = new Set()
    const params = rowsParams()
    params.set('page', String(page))
    params.set('size', size)
    await resource.load(signal => api.get<TableData>(`/tables/${enc(name!)}?${params}`, { signal }))
  }

  // Reload when the table, page, size, sort or filters change.
  $effect(() => {
    void [name, view, page, size, sort?.column, sort?.desc, filtersKey]
    untrack(load)
    return () => resource.cancel()
  })

  function setFilters(next: TableFilter[]) {
    filterOpen = false
    filterPreset = undefined
    const search = tableViewSearch(new URLSearchParams(filtersToSearch(next)), { page: 0, size, sort })
    navigate(`/tables/${enc(name!)}${search ? `?${search}` : ''}`)
  }

  const exportHref = (format: 'csv' | 'json') => {
    const params = rowsParams()
    params.set('format', format)
    return href(`/api/tables/${enc(name!)}/export?${params}`)
  }

  /** Link to the row referenced by the foreign key. */
  const referenceHref = (column: Column, value: string) =>
    href(
      `/tables/${enc(column.references!.table)}?${filtersToSearch([{ column: column.references!.column, op: 'eq', value }])}`,
    )

  function toggleSort(column: string) {
    updateView({ page: 0, sort: sort?.column === column ? (sort.desc ? null : { column, desc: true }) : { column, desc: false } })
  }

  function setSort(column: string, direction: 'asc' | 'desc' | null) {
    updateView({ page: 0, sort: direction ? { column, desc: direction === 'desc' } : null })
  }

  function updateView(patch: Partial<TableView>) {
    const search = tableViewSearch(route.query, patch)
    navigate(`/tables/${enc(name!)}${search ? `?${search}` : ''}`)
  }

  /** Column menu: opens the filter bar with a row for that column. */
  function filterBy(column: string) {
    filterPreset = column
    filterOpen = true
  }

  function pkOf(row: RowData) {
    return rowPk(row, data?.table.primary_key ?? [])
  }

  async function commitCell(pk: RowData, column: string, value: string | null) {
    if (!data || !name || loading || savingCell || data.table.name !== name) return
    const target = data
    const table = name
    const key = rowKey(pk, target.table.primary_key)
    savingCell = true
    try {
      const res = await api.patch<{ count: number }>(`/tables/${enc(table)}/rows`, {
        pk,
        values: { [column]: value },
      })
      if (name === table && data === target) {
        const row = data.rows.find((candidate) => rowKey(candidate, target.table.primary_key) === key)
        if (row && res.count > 0) row[column] = value
      } else if (name === table) {
        await load()
      }
      toast.success(t('tables.toast.rowsUpdated', { count: res.count }))
    } catch (e) {
      toast.error(errorMessage(e))
      throw e
    } finally {
      savingCell = false
    }
  }

  async function deleteSelected() {
    if (!data || !name) return
    const pks = [...selected].map((i) => pkOf(data!.rows[i]))
    try {
      const res = await api.delete<{ count: number }>(`/tables/${enc(name)}/rows`, { pks })
      toast.success(t('tables.toast.rowsDeleted', { count: res.count }))
      await load()
    } catch (e) {
      toast.error(errorMessage(e))
      throw e
    }
  }

  function openSheet(row: RowData | null) {
    sheetRow = row
    sheetOpen = true
  }
</script>

<div class="flex h-full min-h-0">
  <TableSidebar {tables} current={name} loading={tablesLoading} error={tablesError} onretry={loadTables} oncreate={() => (createOpen = true)} />

  <section class={cn('min-w-0 flex-1 flex-col', name ? 'flex' : 'hidden lg:flex')}>
    {#if !name}
      <div class="grid flex-1 place-items-center p-8">
        <EmptyState title={t('tables.editor.noneOpen')} description={t('tables.editor.pickOne')}>
          {#snippet actions()}
            <Button variant="outline" onclick={() => (createOpen = true)}><Plus />{t('tables.editor.newTable')}</Button>
          {/snippet}
        </EmptyState>
      </div>
    {:else}
      <TableToolbar
        {name}
        {view}
        {data}
        filterCount={filters.length}
        bind:filterOpen
        loading={loading || savingCell}
        hiddenColumns={hidden.names}
        ontogglecolumn={(column) => hidden.toggle(column)}
        onshowallcolumns={() => hidden.showAll()}
        {exportHref}
        onreload={load}
        oninsert={() => openSheet(null)}
      />

      {#if view === 'structure'}
        <div class="min-h-0 flex-1 overflow-auto">
          <StructureView {name} onrenamed={onRenamed} ondropped={onDropped} />
        </div>
      {:else}
        {#if filterOpen && data}
          {#key filterPreset}
            <FilterBar
              columns={data.table.columns}
              {filters}
              preset={filterPreset}
              onapply={setFilters}
              onclose={() => {
                filterOpen = false
                filterPreset = undefined
              }}
            />
          {/key}
        {:else if filters.length || sort}
          <FilterChips {filters} {sort} onchange={setFilters} onclearsort={() => setSort('', null)} />
        {/if}

        {#if data && selected.size > 0}
          <SelectionBar
            table={name}
            columns={data.table.columns.map((c) => c.name)}
            rows={[...selected].sort((a, b) => a - b).map((i) => data!.rows[i]).filter(Boolean)}
            deletable={data.table.editable && !loading && !savingCell}
            onclear={() => (selected = new Set())}
            ondelete={() => (confirmOpen = true)}
          />
        {/if}

        {#if data?.table.exposed_without_rls}
          <p class="flex items-center gap-2 border-b border-destructive/30 bg-destructive/5 px-4 py-2 text-sm text-destructive">
            <ShieldAlert class="size-4 shrink-0" />{t('tables.editor.noRls')}
          </p>
        {:else if data && !data.table.editable && data.table.kind === 'table'}
          <p class="flex items-center gap-2 border-b bg-muted/40 px-4 py-2 text-sm text-muted-foreground">
            <KeyRound class="size-4 shrink-0" />{t('tables.editor.noPrimaryKey')}
          </p>
        {/if}

        <div class="relative min-h-0 flex-1 overflow-auto" aria-busy={loading || savingCell}>
          <!-- Reload (order, filter, page): a bar at the top and a dimmed grid,
               so the old data does not look like the new one. -->
          {#if (loading || savingCell) && data}
            <div class="pointer-events-none sticky top-0 z-20 h-0.5 overflow-hidden bg-brand/15" aria-hidden="true">
              <div class="animate-progress h-full w-2/5 bg-brand"></div>
            </div>
          {/if}
          {#if error && !data}
            <GridState state="error" message={error} onretry={load} />
          {:else if !data}
            <GridState state="loading" />
          {:else}
            {#if error}<GridState state="error" message={error} onretry={load} />{/if}
            <div class={['transition-opacity', loading && 'opacity-60']}>
              <DataGrid
                {data}
                disabled={loading || savingCell}
                {sort}
                hidden={hidden.names}
                bind:selected
                onsort={toggleSort}
                onsortset={setSort}
                onfilter={filterBy}
                onhide={(column) => hidden.hide(column)}
                onexpand={openSheet}
                oncommit={commitCell}
                {referenceHref}
              />
            </div>
            {#if data.rows.length === 0}
              {#if filters.length}
                <GridState state="no-match" onclearfilters={() => setFilters([])} />
              {:else}
                <GridState state="empty" insertable={data.table.insertable} oninsert={() => openSheet(null)} />
              {/if}
            {/if}
          {/if}
        </div>

        {#if data}<GridFooter {data} {page} {size} disabled={loading || savingCell} onpage={(page) => updateView({ page })} onsize={(size) => updateView({ page: 0, size })} />{/if}
      {/if}
    {/if}
  </section>
</div>

<CreateTableSheet bind:open={createOpen} oncreated={onCreated} />

{#if data && name}
  <RowSheet
    bind:open={sheetOpen}
    table={name}
    columns={data.table.columns}
    primaryKey={data.table.primary_key}
    row={sheetRow}
    onsaved={load}
  />
  <ConfirmDialog
    bind:open={confirmOpen}
    title={t('tables.editor.deleteTitle', { count: selected.size })}
    description={t('tables.editor.deleteDescription')}
    confirmLabel={t('common.delete')}
    destructive
    onconfirm={deleteSelected}
  />
{/if}
