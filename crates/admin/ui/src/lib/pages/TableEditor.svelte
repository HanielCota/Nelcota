<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import Plus from '@lucide/svelte/icons/plus'
  import Table2 from '@lucide/svelte/icons/table-2'
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
  import { api, enc, isAbort } from '$lib/api'
  import { HiddenColumns } from '$lib/hidden-columns.svelte'
  import { filtersParam, filtersToSearch, parseFilters, type TableFilter } from '$lib/filters'
  import { href, navigate, route } from '$lib/router.svelte'
  import { cn } from '$lib/utils'
  import type { Column, RowData, TableData, TableSummary } from '$lib/types'

  let { name, view = 'data' }: { name?: string; view?: 'data' | 'structure' } = $props()

  let tables = $state<TableSummary[]>([])
  let data = $state<TableData | null>(null)
  let loading = $state(false)
  let error = $state('')
  let page = $state(0)
  let size = $state('50')
  let sort = $state<{ column: string; desc: boolean } | null>(null)
  let selected = $state<Set<number>>(new Set())

  let sheetOpen = $state(false)
  let sheetRow = $state<RowData | null>(null)
  let confirmOpen = $state(false)
  let filterOpen = $state(false)
  let filterPreset = $state<string | undefined>(undefined)
  let createOpen = $state(false)

  // Colunas ocultas, lembradas por tabela.
  const hidden = new HiddenColumns()
  $effect(() => {
    if (name) hidden.load(name)
  })
  $effect(() => {
    if (data) hidden.prune(data.table.columns.map((c) => c.name))
  })

  // Filtros vêm da URL (`?preco=gte.10`): links compartilháveis e o voltar do
  // navegador funcionam. A chave em texto evita recarregar sem mudança real.
  const filters = $derived(parseFilters(route.query))
  const filtersKey = $derived(filtersToSearch(filters))

  async function loadTables() {
    try {
      tables = (await api.get<{ tables: TableSummary[] }>('/tables')).tables
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  onMount(loadTables)

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

  /** Ordem e filtros atuais, no formato da API (listagem e exportação). */
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

  let inflight: AbortController | undefined

  async function load() {
    if (!name) return
    // Só a resposta mais recente vale: a anterior é cancelada.
    inflight?.abort()
    const controller = (inflight = new AbortController())
    loading = true
    error = ''
    try {
      const params = rowsParams()
      params.set('page', String(page))
      params.set('size', size)
      data = await api.get<TableData>(`/tables/${enc(name)}?${params}`, { signal: controller.signal })
      selected = new Set()
    } catch (e) {
      if (isAbort(e)) return
      error = (e as Error).message
      data = null
    } finally {
      if (inflight === controller) loading = false
    }
  }

  // Recarrega ao trocar de tabela, página, tamanho, ordenação ou filtros.
  $effect(() => {
    void [name, view, page, size, sort, filtersKey]
    load()
    return () => inflight?.abort()
  })

  function setFilters(next: TableFilter[]) {
    filterOpen = false
    filterPreset = undefined
    page = 0
    const search = filtersToSearch(next)
    navigate(`/tables/${enc(name!)}${search ? `?${search}` : ''}`)
  }

  const exportHref = (format: 'csv' | 'json') => {
    const params = rowsParams()
    params.set('format', format)
    return href(`/api/tables/${enc(name!)}/export?${params}`)
  }

  /** Link para a linha referenciada pela chave estrangeira. */
  const referenceHref = (column: Column, value: string) =>
    href(
      `/tables/${enc(column.references!.table)}?${filtersToSearch([{ column: column.references!.column, op: 'eq', value }])}`,
    )

  function toggleSort(column: string) {
    sort = sort?.column === column ? (sort.desc ? null : { column, desc: true }) : { column, desc: false }
    page = 0
  }

  function setSort(column: string, direction: 'asc' | 'desc' | null) {
    sort = direction ? { column, desc: direction === 'desc' } : null
    page = 0
  }

  /** Menu da coluna: abre a barra de filtros com uma linha para ela. */
  function filterBy(column: string) {
    filterPreset = column
    filterOpen = true
  }

  function pkOf(row: RowData) {
    return Object.fromEntries((data?.table.primary_key ?? []).map((k) => [k, row[k]]))
  }

  async function commitCell(row: number, column: string, value: string | null) {
    if (!data || !name) return
    try {
      const res = await api.patch<{ message: string }>(`/tables/${enc(name)}/rows`, {
        pk: pkOf(data.rows[row]),
        values: { [column]: value },
      })
      data.rows[row][column] = value
      toast.success(res.message)
    } catch (e) {
      toast.error((e as Error).message)
      throw e
    }
  }

  async function deleteSelected() {
    if (!data || !name) return
    const pks = [...selected].map((i) => pkOf(data!.rows[i]))
    try {
      const res = await api.delete<{ message: string }>(`/tables/${enc(name)}/rows`, { pks })
      toast.success(res.message)
      await load()
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  function openSheet(row: RowData | null) {
    sheetRow = row
    sheetOpen = true
  }
</script>

<div class="flex h-full min-h-0">
  <TableSidebar {tables} current={name} oncreate={() => (createOpen = true)} />

  <section class={cn('min-w-0 flex-1 flex-col', name ? 'flex' : 'hidden md:flex')}>
    {#if !name}
      <div class="grid flex-1 place-items-center p-8">
        <EmptyState
          icon={Table2}
          title="Escolha uma tabela"
          description="Selecione na lista ao lado para ver e editar as linhas, ou crie uma nova."
          class="w-full max-w-lg"
        >
          {#snippet actions()}
            <Button onclick={() => (createOpen = true)}><Plus />Criar tabela</Button>
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
        {loading}
        hiddenColumns={hidden.names}
        ontogglecolumn={(column) => hidden.toggle(column)}
        onshowallcolumns={() => hidden.showAll()}
        {exportHref}
        onreload={load}
        oninsert={() => openSheet(null)}
      />

      {#if view === 'structure'}
        <div class="min-h-0 flex-1 overflow-auto bg-muted/20">
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
            rows={[...selected].sort((a, b) => a - b).map((i) => data!.rows[i])}
            deletable={data.table.editable}
            onclear={() => (selected = new Set())}
            ondelete={() => (confirmOpen = true)}
          />
        {/if}

        {#if data?.table.exposed_without_rls}
          <p class="flex items-center gap-2 border-b border-destructive/30 bg-destructive/5 px-4 py-2.5 text-sm text-destructive">
            <ShieldAlert class="size-4 shrink-0" />
            <span><span class="font-semibold">Sem RLS:</span> quem tem GRANT nesta tabela lê e altera todas as linhas.</span>
          </p>
        {:else if data && !data.table.editable && data.table.kind === 'table'}
          <p class="flex items-center gap-2 border-b bg-muted/40 px-4 py-2.5 text-sm text-muted-foreground">
            <KeyRound class="size-4 shrink-0" />
            <span><span class="font-semibold text-foreground">Sem chave primária:</span> dá para ver as linhas, mas não editar por aqui.</span>
          </p>
        {/if}

        <div class="relative min-h-0 flex-1 overflow-auto" aria-busy={loading}>
          <!-- Recarga (ordem, filtro, página): barra no topo e grade esmaecida,
               para os dados antigos não parecerem já os novos. -->
          {#if loading && data}
            <div class="pointer-events-none sticky top-0 z-20 h-[3px] overflow-hidden bg-brand/15" aria-hidden="true">
              <div class="animate-progress h-full w-2/5 bg-brand"></div>
            </div>
          {/if}
          {#if error}
            <GridState state="error" message={error} onretry={load} />
          {:else if !data}
            <GridState state="loading" />
          {:else}
            <div class={['transition-opacity', loading && 'opacity-60']}>
              <DataGrid
                {data}
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

        {#if data}<GridFooter {data} bind:page bind:size />{/if}
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
    title={selected.size === 1 ? 'Apagar 1 linha?' : `Apagar ${selected.size} linhas?`}
    description="Esta ação não pode ser desfeita. As linhas são apagadas numa transação só."
    confirmLabel="Apagar"
    destructive
    onconfirm={deleteSelected}
  />
{/if}
