<script lang="ts">
  import { tick } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import Pencil from '@lucide/svelte/icons/pencil'
  import ArrowUp from '@lucide/svelte/icons/arrow-up'
  import ArrowDown from '@lucide/svelte/icons/arrow-down'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import { formatCell } from '$lib/format'
  import type { Column, RowData, TableData } from '$lib/types'

  let {
    data,
    sort,
    selected = $bindable(),
    onsort,
    onexpand,
    oncommit,
    referenceHref,
  }: {
    data: TableData
    sort: { column: string; desc: boolean } | null
    selected: Set<number>
    onsort: (column: string) => void
    onexpand: (row: RowData) => void
    /** Salva uma célula; resolve depois de gravar (ou rejeita com o erro já avisado). */
    oncommit: (row: number, column: string, value: string | null) => Promise<void>
    referenceHref: (column: Column, value: string) => string
  } = $props()

  const columns = $derived(data.table.columns)
  const editable = $derived(data.table.editable)

  // Edição inline: célula (linha, coluna) em edição e o rascunho.
  let editing = $state<{ row: number; column: string } | null>(null)
  let draft = $state('')

  async function startEdit(rowIndex: number, column: Column) {
    if (!editable || column.generated) return
    editing = { row: rowIndex, column: column.name }
    draft = data.rows[rowIndex][column.name] ?? ''
    await tick()
    document.getElementById('inline-editor')?.focus()
  }

  async function commitEdit(asNull = false) {
    if (!editing) return
    const { row, column } = editing
    const value = asNull ? null : draft
    editing = null
    if (value === data.rows[row][column]) return
    await oncommit(row, column, value).catch(() => {})
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
    selected = on ? new Set(data.rows.map((_, i) => i)) : new Set()
  }
</script>

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
            onclick={() => onsort(column.name)}
            title={column.comment ?? `Ordenar por ${column.name}`}
          >
            <span class="grid">
              <span class="text-xs text-foreground"
                >{column.name}{#if column.is_pk}<span class="ml-1.5 font-mono text-3xs font-normal text-muted-foreground">pk</span>{/if}</span
              >
              <span class="font-mono text-3xs font-normal text-muted-foreground"
                >{column.full_type}{#if column.references}<span class="text-brand">{` → ${column.references.table}`}</span
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
            class={['group/cell max-w-96 border-r border-b p-0 font-mono', editable && !column.generated && 'cursor-text']}
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
                  {#if value === null}<span class="text-muted-foreground">NULL</span>{:else}{formatCell(value, column.type)
                      .text}{/if}
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
              onclick={() => onexpand(row)}
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
