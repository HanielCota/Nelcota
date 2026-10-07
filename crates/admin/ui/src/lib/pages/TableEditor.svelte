<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import * as Select from '$lib/components/ui/select'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import Search from '@lucide/svelte/icons/search'
  import RefreshCw from '@lucide/svelte/icons/refresh-cw'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import Pencil from '@lucide/svelte/icons/pencil'
  import Plus from '@lucide/svelte/icons/plus'
  import Table2 from '@lucide/svelte/icons/table-2'
  import ArrowUp from '@lucide/svelte/icons/arrow-up'
  import ArrowDown from '@lucide/svelte/icons/arrow-down'
  import ChevronLeft from '@lucide/svelte/icons/chevron-left'
  import ChevronRight from '@lucide/svelte/icons/chevron-right'
  import Funnel from '@lucide/svelte/icons/funnel'
  import Download from '@lucide/svelte/icons/download'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import X from '@lucide/svelte/icons/x'
  import { toast } from 'svelte-sonner'
  import RlsBadge from '$lib/components/app/RlsBadge.svelte'
  import RlsDot from '$lib/components/app/RlsDot.svelte'
  import RowSheet from '$lib/components/app/RowSheet.svelte'
  import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte'
  import FilterBar from '$lib/components/app/FilterBar.svelte'
  import CreateTableSheet from '$lib/components/app/CreateTableSheet.svelte'
  import StructureView from '$lib/components/app/StructureView.svelte'
  import { api, enc, isAbort } from '$lib/api'
  import { describe, filtersParam, filtersToSearch, parseFilters, type TableFilter } from '$lib/filters'
  import { href, navigate, route } from '$lib/router.svelte'
  import { cn } from '$lib/utils'
  import type { Column, RowData, TableData, TableSummary } from '$lib/types'

  let { name, view = 'data' }: { name?: string; view?: 'data' | 'structure' } = $props()

  let tables = $state<TableSummary[]>([])
  let filter = $state('')
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

  // Filtros vêm da URL (`?preco=gte.10`): links compartilháveis e o voltar do
  // navegador funcionam. A chave em texto evita recarregar sem mudança real.
  const filters = $derived(parseFilters(route.query))
  const filtersKey = $derived(filtersToSearch(filters))

  // Edição inline: célula (linha, coluna) em edição e o rascunho.
  let editing = $state<{ row: number; column: string } | null>(null)
  let draft = $state('')

  const visibleTables = $derived(
    tables.filter((t) => t.name.toLowerCase().includes(filter.trim().toLowerCase())),
  )
  const columns = $derived(data?.table.columns ?? [])
  const editable = $derived(data?.table.editable ?? false)
  const fmt = new Intl.NumberFormat('pt-BR')

  async function loadTables() {
    try {
      tables = (await api.get<{ tables: TableSummary[] }>('/tables')).tables
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  onMount(loadTables)

  let createOpen = $state(false)

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

  const tabs = [
    { view: 'data', label: 'Dados', suffix: '' },
    { view: 'structure', label: 'Estrutura', suffix: '/structure' },
  ] as const

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

  function pkOf(row: RowData) {
    return Object.fromEntries((data?.table.primary_key ?? []).map((k) => [k, row[k]]))
  }

  async function startEdit(rowIndex: number, column: Column) {
    if (!editable || column.generated) return
    editing = { row: rowIndex, column: column.name }
    draft = data!.rows[rowIndex][column.name] ?? ''
    await tick()
    document.getElementById('inline-editor')?.focus()
  }

  async function commitEdit(asNull = false) {
    if (!editing || !data || !name) return
    const { row, column } = editing
    const original = data.rows[row][column]
    const value = asNull ? null : draft
    editing = null
    if (value === original) return
    try {
      const res = await api.patch<{ message: string }>(`/tables/${enc(name)}/rows`, {
        pk: pkOf(data.rows[row]),
        values: { [column]: value },
      })
      data.rows[row][column] = value
      toast.success(res.message)
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  function onEditorKey(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      commitEdit()
    } else if (event.key === 'Escape') {
      editing = null
    }
  }

  function toggleRow(index: number, on: boolean) {
    const next = new Set(selected)
    if (on) next.add(index)
    else next.delete(index)
    selected = next
  }

  function toggleAll(on: boolean) {
    selected = on && data ? new Set(data.rows.map((_, i) => i)) : new Set()
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
  <!-- Lista de tabelas -->
  <!-- No celular, sem tabela escolhida a lista ocupa a tela; com tabela, só a grade. -->
  <aside
    aria-label="Lista de tabelas"
    class={cn('w-full shrink-0 flex-col border-r bg-sidebar md:flex md:w-64', name ? 'hidden' : 'flex')}
  >
    <div class="grid gap-3 border-b p-3">
      <div class="flex items-center justify-between">
        <svelte:element this={name ? 'p' : 'h1'} class="px-1 text-sm font-medium">Editor de tabelas</svelte:element>
        <Button variant="ghost" size="icon-sm" title="Nova tabela" aria-label="Nova tabela" onclick={() => (createOpen = true)}>
          <Plus />
        </Button>
      </div>
      <div class="relative">
        <Search class="absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input bind:value={filter} placeholder="Buscar tabelas…" class="h-8 bg-card pl-8 text-sm" />
      </div>
    </div>
    <p class="px-4 pt-3 pb-1 text-2xs font-medium tracking-wider text-muted-foreground uppercase">
      Tabelas ({visibleTables.length})
    </p>
    <nav class="flex-1 overflow-y-auto px-2 pb-2" aria-label="Tabelas">
      {#each visibleTables as table (table.name)}
        <a
          href={href(`/tables/${encodeURIComponent(table.name)}`)}
          title={table.rls.label}
          aria-current={table.name === name ? 'page' : undefined}
          class={cn(
            'flex h-10 items-center gap-2 rounded-md px-2 text-sm text-muted-foreground md:h-8 transition-colors hover:bg-accent/60 hover:text-foreground',
            table.name === name && 'bg-accent text-foreground',
          )}
        >
          <Table2 class={['size-3.5 shrink-0', table.name === name && 'text-brand']} strokeWidth={1.6} />
          <span class="truncate">{table.name}</span>
          <span class="ml-auto flex"><RlsDot state={table.rls.state} /></span>
          {#if table.kind !== 'table'}<span class="text-2xs">view</span>{/if}
        </a>
      {:else}
        <p class="px-2 py-4 text-sm text-muted-foreground">Nenhuma tabela.</p>
      {/each}
    </nav>
  </aside>

  <!-- Área principal -->
  <section class={cn('min-w-0 flex-1 flex-col', name ? 'flex' : 'hidden md:flex')}>
    {#if !name}
      <div class="grid flex-1 place-items-center p-8 text-center">
        <div>
          <Table2 class="mx-auto size-7 text-muted-foreground" strokeWidth={1.3} />
          <h2 class="mt-3 text-sm font-medium">Escolha uma tabela</h2>
          <p class="mt-1 text-sm font-light text-muted-foreground">Selecione na lista ao lado para ver e editar as linhas.</p>
          <Button variant="outline" size="sm" class="mt-4" onclick={() => (createOpen = true)}><Plus />Criar tabela</Button>
        </div>
      </div>
    {:else}
      <div class="flex min-h-12 shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b px-4 py-2">
        <a
          href={href('/tables')}
          class="-ml-1 grid size-7 place-items-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground md:hidden"
          aria-label="Voltar para a lista de tabelas"><ChevronLeft class="size-4" /></a
        >
        <h1 class="min-w-0 truncate text-sm font-medium">{name}</h1>
        {#if data}<RlsBadge rls={data.table.rls} />{/if}
        <nav class="ml-1 flex items-center gap-0.5 rounded-md bg-muted p-0.5 text-xs" aria-label="Visão da tabela">
          {#each tabs as tab (tab.view)}
            <a
              href={href(`/tables/${enc(name)}${tab.suffix}`)}
              aria-current={view === tab.view ? 'page' : undefined}
              class={[
                'rounded px-2.5 py-1 transition-colors',
                view === tab.view ? 'bg-card text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground',
              ]}>{tab.label}</a
            >
          {/each}
        </nav>
        {#if view === 'data'}
          <div class="ml-auto flex flex-wrap items-center gap-2">
            {#if selected.size > 0}
              <Button variant="destructive" size="sm" onclick={() => (confirmOpen = true)}>
                <Trash2 />Apagar {selected.size}
              </Button>
            {/if}
            <Button
              variant={filterOpen || filters.length ? 'secondary' : 'ghost'}
              size="sm"
              onclick={() => (filterOpen = !filterOpen)}
              aria-expanded={filterOpen}
            >
              <Funnel />Filtrar{#if filters.length}<span
                  class="rounded-full bg-brand/15 px-1.5 text-3xs text-brand tabular-nums">{filters.length}</span
                >{/if}
            </Button>
            <DropdownMenu.Root>
              <DropdownMenu.Trigger>
                {#snippet child({ props })}
                  <Button variant="ghost" size="sm" {...props}><Download />Exportar</Button>
                {/snippet}
              </DropdownMenu.Trigger>
              <DropdownMenu.Content align="end" class="w-56">
                <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">
                  {filters.length ? 'Linhas filtradas, na ordem atual' : 'Todas as linhas, na ordem atual'}
                </DropdownMenu.Label>
                <DropdownMenu.Item>
                  {#snippet child({ props })}<a {...props} href={exportHref('csv')} download>CSV</a>{/snippet}
                </DropdownMenu.Item>
                <DropdownMenu.Item>
                  {#snippet child({ props })}<a {...props} href={exportHref('json')} download>JSON</a>{/snippet}
                </DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Root>
            <Button variant="ghost" size="icon-sm" onclick={load} aria-label="Recarregar" title="Recarregar">
              <RefreshCw class={loading ? 'animate-spin' : ''} />
            </Button>
            {#if data?.table.insertable}
              <Button size="sm" onclick={() => openSheet(null)}><Plus />Inserir linha</Button>
            {/if}
          </div>
        {/if}
      </div>

      {#if view === 'structure'}
        <div class="min-h-0 flex-1 overflow-auto bg-muted/20">
          <StructureView {name} onrenamed={onRenamed} ondropped={onDropped} />
        </div>
      {:else}
        {#if filterOpen && data}
          <FilterBar columns={data.table.columns} {filters} onapply={setFilters} onclose={() => (filterOpen = false)} />
        {:else if filters.length}
          <div class="flex shrink-0 flex-wrap items-center gap-1.5 border-b px-4 py-2">
            {#each filters as filter, i (i)}
              <span
                class="inline-flex h-6 items-center gap-1 rounded-md border border-border-strong bg-muted pr-0.5 pl-2 font-mono text-2xs"
              >
                {describe(filter)}
                <button
                  class="grid size-5 place-items-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
                  aria-label="Remover filtro"
                  onclick={() => setFilters(filters.filter((_, j) => j !== i))}><X class="size-3" /></button
                >
              </span>
            {/each}
            <Button variant="ghost" size="xs" class="text-muted-foreground" onclick={() => setFilters([])}>Limpar</Button>
          </div>
      {/if}

      {#if data?.table.exposed_without_rls}
        <p class="border-b border-destructive/30 bg-destructive/5 px-4 py-2 text-xs text-destructive">
          Sem RLS: quem tem GRANT nesta tabela lê e altera todas as linhas.
        </p>
      {:else if data && !data.table.editable && data.table.kind === 'table'}
        <p class="border-b bg-muted/40 px-4 py-2 text-xs text-muted-foreground">
          Sem chave primária: dá para ver as linhas, mas não editar por aqui.
        </p>
      {/if}

      <div class="min-h-0 flex-1 overflow-auto">
        {#if error}
          <p class="p-4 text-sm text-destructive">{error}</p>
        {:else if !data}
          <p class="p-4 text-sm text-muted-foreground">Carregando…</p>
        {:else}
          <table class="w-max min-w-full border-separate border-spacing-0 text-xs">
            <thead class="sticky top-0 z-10">
              <tr>
                {#if editable}
                  <th class="w-10 border-r border-b bg-card px-3 py-2">
                    <Checkbox
                      checked={selected.size > 0 && selected.size === data.rows.length}
                      indeterminate={selected.size > 0 && selected.size < data.rows.length}
                      onCheckedChange={(v) => toggleAll(v === true)}
                      aria-label="Selecionar todas"
                    />
                  </th>
                {/if}
                {#each columns as column (column.name)}
                  <th class="border-r border-b bg-card p-0 text-left font-medium">
                    <button
                      class="group flex w-full min-w-36 items-start gap-1.5 px-3 py-2 text-left hover:bg-accent"
                      onclick={() => toggleSort(column.name)}
                      title={column.comment ?? `Ordenar por ${column.name}`}
                    >
                      <span class="grid">
                        <span class="text-xs text-foreground"
                          >{column.name}{#if column.is_pk}<span class="ml-1.5 font-mono text-3xs font-normal text-muted-foreground">pk</span>{/if}</span
                        >
                        <span class="font-mono text-3xs font-normal text-muted-foreground"
                          >{column.full_type}{#if column.references}<span class="text-brand"
                              >{` → ${column.references.table}`}</span
                            >{/if}</span
                        >
                      </span>
                      {#if sort?.column === column.name}
                        {#if sort.desc}<ArrowDown class="ml-auto size-3.5" />{:else}<ArrowUp class="ml-auto size-3.5" />{/if}
                      {/if}
                    </button>
                  </th>
                {/each}
                {#if editable}<th class="w-12 border-b bg-card"><span class="sr-only">Ações</span></th>{/if}
              </tr>
            </thead>
            <tbody>
              {#each data.rows as row, i (i)}
                <tr class={['group', selected.has(i) ? 'bg-brand/5' : 'hover:bg-muted/50']}>
                  {#if editable}
                    <td class="border-r border-b px-3 py-1.5">
                      <Checkbox checked={selected.has(i)} onCheckedChange={(v) => toggleRow(i, v === true)} aria-label="Selecionar linha" />
                    </td>
                  {/if}
                  {#each columns as column (column.name)}
                    {@const value = row[column.name]}
                    <td
                      class={[
                        'group/cell max-w-96 border-r border-b p-0 font-mono',
                        editable && !column.generated && 'cursor-text',
                      ]}
                      ondblclick={() => startEdit(i, column)}
                    >
                      {#if editing?.row === i && editing.column === column.name}
                        <div class="flex items-center gap-1 bg-background p-0.5 ring-1 ring-brand/70 ring-inset">
                          <input
                            id="inline-editor"
                            class="w-full min-w-40 bg-transparent px-2 py-1 font-mono text-xs outline-none"
                            bind:value={draft}
                            onkeydown={onEditorKey}
                            onblur={() => (editing = null)}
                          />
                          {#if column.nullable}
                            <button
                              class="shrink-0 rounded border px-1.5 text-3xs text-muted-foreground hover:text-foreground"
                              onmousedown={(e) => {
                                e.preventDefault()
                                commitEdit(true)
                              }}>NULL</button
                            >
                          {/if}
                        </div>
                      {:else}
                        <div class="flex items-center gap-1 px-3 py-1.5" title={value ?? 'NULL'}>
                          <span class="truncate">
                            {#if value === null}<span class="text-muted-foreground">NULL</span>{:else}{value}{/if}
                          </span>
                          {#if column.references && value !== null}
                            <a
                              href={referenceHref(column, value)}
                              class="ml-auto grid size-5 shrink-0 place-items-center rounded text-muted-foreground opacity-0 group-hover/cell:opacity-100 hover:bg-accent hover:text-brand focus-visible:opacity-100"
                              title={`Abrir em ${column.references.table}`}
                              aria-label={`Abrir linha referenciada em ${column.references.table}`}
                              ondblclick={(e) => e.stopPropagation()}
                            >
                              <ArrowUpRight class="size-3.5" />
                            </a>
                          {/if}
                        </div>
                      {/if}
                    </td>
                  {/each}
                  {#if editable}
                    <td class="border-b px-2 text-right">
                      <Button
                        variant="ghost"
                        size="icon-xs"
                        class="opacity-0 group-hover:opacity-100"
                        onclick={() => openSheet(row)}
                        aria-label="Editar linha"
                      >
                        <Pencil />
                      </Button>
                    </td>
                  {/if}
                </tr>
              {/each}
            </tbody>
          </table>
          {#if data.rows.length === 0}
            <p class="py-12 text-center text-sm text-muted-foreground">
              {filters.length ? 'Nenhuma linha para esses filtros.' : 'Nenhuma linha.'}
            </p>
          {/if}
        {/if}
      </div>

      {#if data}
        <footer class="flex shrink-0 flex-wrap items-center gap-3 border-t bg-sidebar px-4 py-2 text-xs text-muted-foreground">
          <span>
            {#if data.total !== null}{data.total_exact ? '' : '~'}{fmt.format(data.total)} {data.total === 1 ? 'linha' : 'linhas'}{:else}{data.rows.length} nesta página{/if}
          </span>
          {#if editable}<span class="hidden lg:inline">Duplo clique numa célula para editar.</span>{/if}
          <div class="ml-auto flex items-center gap-2">
            <span>Por página</span>
            <Select.Root type="single" bind:value={size} onValueChange={() => (page = 0)}>
              <Select.Trigger size="sm" class="h-7 w-20">{size}</Select.Trigger>
              <Select.Content>
                {#each ['25', '50', '100', '500'] as option (option)}
                  <Select.Item value={option}>{option}</Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
            <span class="px-1">Página {data.page + 1}</span>
            <Button variant="outline" size="icon-sm" disabled={page === 0} onclick={() => page--} aria-label="Anterior">
              <ChevronLeft />
            </Button>
            <Button variant="outline" size="icon-sm" disabled={!data.has_next} onclick={() => page++} aria-label="Próxima">
              <ChevronRight />
            </Button>
          </div>
        </footer>
      {/if}
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
    title={`Apagar ${selected.size} linha(s)?`}
    description="Esta ação não pode ser desfeita. As linhas são apagadas numa transação só."
    confirmLabel="Apagar"
    destructive
    onconfirm={deleteSelected}
  />
{/if}
