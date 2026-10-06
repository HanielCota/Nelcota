<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import * as Select from '$lib/components/ui/select'
  import Search from '@lucide/svelte/icons/search'
  import RefreshCw from '@lucide/svelte/icons/refresh-cw'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import Pencil from '@lucide/svelte/icons/pencil'
  import ArrowUp from '@lucide/svelte/icons/arrow-up'
  import ArrowDown from '@lucide/svelte/icons/arrow-down'
  import ChevronLeft from '@lucide/svelte/icons/chevron-left'
  import ChevronRight from '@lucide/svelte/icons/chevron-right'
  import { toast } from 'svelte-sonner'
  import RlsBadge from '$lib/components/app/RlsBadge.svelte'
  import RlsDot from '$lib/components/app/RlsDot.svelte'
  import RowSheet from '$lib/components/app/RowSheet.svelte'
  import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte'
  import { api, enc } from '$lib/api'
  import { href } from '$lib/router.svelte'
  import type { Column, RowData, TableData, TableSummary } from '$lib/types'

  let { name }: { name?: string } = $props()

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

  // Edição inline: célula (linha, coluna) em edição e o rascunho.
  let editing = $state<{ row: number; column: string } | null>(null)
  let draft = $state('')

  const visibleTables = $derived(
    tables.filter((t) => t.name.toLowerCase().includes(filter.trim().toLowerCase())),
  )
  const columns = $derived(data?.table.columns ?? [])
  const editable = $derived(data?.table.editable ?? false)
  const fmt = new Intl.NumberFormat('pt-BR')

  onMount(async () => {
    try {
      tables = (await api.get<{ tables: TableSummary[] }>('/tables')).tables
    } catch (e) {
      toast.error((e as Error).message)
    }
  })

  async function load() {
    if (!name) return
    loading = true
    error = ''
    try {
      const params = new URLSearchParams({ page: String(page), size })
      if (sort) {
        params.set('sort', sort.column)
        if (sort.desc) params.set('desc', 'true')
      }
      data = await api.get<TableData>(`/tables/${enc(name)}?${params}`)
      selected = new Set()
    } catch (e) {
      error = (e as Error).message
      data = null
    } finally {
      loading = false
    }
  }

  // Recarrega ao trocar de tabela, página, tamanho ou ordenação.
  $effect(() => {
    void [name, page, size, sort]
    load()
  })

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
  <aside class="hidden w-60 shrink-0 flex-col border-r bg-sidebar/50 md:flex">
    <div class="border-b p-3">
      <div class="relative">
        <Search class="absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input bind:value={filter} placeholder="Buscar tabelas…" class="h-8 pl-8 text-sm" />
      </div>
    </div>
    <nav class="flex-1 overflow-y-auto p-2">
      {#each visibleTables as table (table.name)}
        <a
          href={href(`/tables/${encodeURIComponent(table.name)}`)}
          title={table.rls.label}
          class={[
            'flex items-center gap-2 rounded px-2 py-1 text-sm text-muted-foreground hover:bg-accent hover:text-foreground',
            table.name === name && 'bg-accent font-medium text-foreground',
          ]}
        >
          <RlsDot state={table.rls.state} />
          <span class="truncate">{table.name}</span>
          {#if table.kind !== 'table'}<span class="ml-auto text-xs">view</span>{/if}
        </a>
      {:else}
        <p class="px-2 py-4 text-sm text-muted-foreground">Nenhuma tabela.</p>
      {/each}
    </nav>
  </aside>

  <!-- Área principal -->
  <section class="flex min-w-0 flex-1 flex-col">
    {#if !name}
      <p class="p-6 text-sm text-muted-foreground">Escolha uma tabela na lista.</p>
    {:else}
      <div class="flex shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2.5">
        <h1 class="font-semibold">{name}</h1>
        {#if data}<RlsBadge rls={data.table.rls} />{/if}
        <div class="ml-auto flex items-center gap-2">
          {#if selected.size > 0}
            <Button variant="destructive" size="sm" onclick={() => (confirmOpen = true)}>
              <Trash2 />Apagar {selected.size}
            </Button>
          {/if}
          <Button variant="ghost" size="icon-sm" onclick={load} aria-label="Recarregar" title="Recarregar">
            <RefreshCw class={loading ? 'animate-spin' : ''} />
          </Button>
          {#if data?.table.insertable}
            <Button size="sm" onclick={() => openSheet(null)}>Inserir linha</Button>
          {/if}
        </div>
      </div>

      {#if data?.table.exposed_without_rls}
        <p class="border-b px-4 py-2 text-sm text-destructive">
          Sem RLS: quem tem GRANT nesta tabela lê e altera todas as linhas.
        </p>
      {:else if data && !data.table.editable && data.table.kind === 'table'}
        <p class="border-b px-4 py-2 text-sm text-muted-foreground">
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
                  <th class="w-10 border-r border-b bg-muted px-3 py-2">
                    <Checkbox
                      checked={selected.size > 0 && selected.size === data.rows.length}
                      indeterminate={selected.size > 0 && selected.size < data.rows.length}
                      onCheckedChange={(v) => toggleAll(v === true)}
                      aria-label="Selecionar todas"
                    />
                  </th>
                {/if}
                {#each columns as column (column.name)}
                  <th class="border-r border-b bg-muted p-0 text-left font-medium">
                    <button
                      class="group flex w-full min-w-36 items-start gap-1.5 px-3 py-2 text-left hover:bg-accent"
                      onclick={() => toggleSort(column.name)}
                      title={column.comment ?? `Ordenar por ${column.name}`}
                    >
                      <span class="grid">
                        <span class="text-[12.5px] text-foreground"
                          >{column.name}{#if column.is_pk}<span class="ml-1.5 font-mono text-[10px] font-normal text-muted-foreground">pk</span>{/if}</span
                        >
                        <span class="font-mono text-[10.5px] font-normal text-muted-foreground">{column.full_type}</span>
                      </span>
                      {#if sort?.column === column.name}
                        {#if sort.desc}<ArrowDown class="ml-auto size-3.5" />{:else}<ArrowUp class="ml-auto size-3.5" />{/if}
                      {/if}
                    </button>
                  </th>
                {/each}
                {#if editable}<th class="w-12 border-b bg-muted"></th>{/if}
              </tr>
            </thead>
            <tbody>
              {#each data.rows as row, i (i)}
                <tr class={['group', selected.has(i) ? 'bg-muted/60' : 'hover:bg-muted/40']}>
                  {#if editable}
                    <td class="border-r border-b px-3 py-1.5">
                      <Checkbox checked={selected.has(i)} onCheckedChange={(v) => toggleRow(i, v === true)} aria-label="Selecionar linha" />
                    </td>
                  {/if}
                  {#each columns as column (column.name)}
                    {@const value = row[column.name]}
                    <td
                      class={[
                        'max-w-96 border-r border-b p-0 font-mono',
                        editable && !column.generated && 'cursor-text',
                      ]}
                      ondblclick={() => startEdit(i, column)}
                    >
                      {#if editing?.row === i && editing.column === column.name}
                        <div class="flex items-center gap-1 bg-background p-0.5 ring-1 ring-foreground/40 ring-inset">
                          <input
                            id="inline-editor"
                            class="w-full min-w-40 bg-transparent px-2 py-1 font-mono text-xs outline-none"
                            bind:value={draft}
                            onkeydown={onEditorKey}
                            onblur={() => (editing = null)}
                          />
                          {#if column.nullable}
                            <button
                              class="shrink-0 rounded border px-1.5 text-[10px] text-muted-foreground hover:text-foreground"
                              onmousedown={(e) => {
                                e.preventDefault()
                                commitEdit(true)
                              }}>NULL</button
                            >
                          {/if}
                        </div>
                      {:else}
                        <div class="truncate px-3 py-1.5" title={value ?? 'NULL'}>
                          {#if value === null}
                            <span class="text-muted-foreground">NULL</span>
                          {:else}
                            {value}
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
            <p class="py-12 text-center text-sm text-muted-foreground">Nenhuma linha.</p>
          {/if}
        {/if}
      </div>

      {#if data}
        <footer class="flex shrink-0 flex-wrap items-center gap-3 border-t px-4 py-2 text-xs text-muted-foreground">
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
  </section>
</div>

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
