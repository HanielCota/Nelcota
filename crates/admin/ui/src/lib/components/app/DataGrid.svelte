<script lang="ts">
  import { tick } from 'svelte'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import Maximize2 from '@lucide/svelte/icons/maximize-2'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import GridCell from './GridCell.svelte'
  import GridColumnHeader from './GridColumnHeader.svelte'
  import { alignRight, columnKind, columnWidth, monospace } from '$lib/grid'
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

  const columns = $derived(data.table.columns.map((c) => ({ column: c, kind: columnKind(c), width: columnWidth(c) })))
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

  const sortOf = (name: string) => (sort?.column === name ? (sort.desc ? 'desc' : 'asc') : null)

  // Coluna fixa à esquerda (seleção + expandir): fundo opaco para o conteúdo
  // que rola por baixo não aparecer através dela.
  const stickyCell = 'sticky left-0 z-[1] border-r border-b bg-background'
</script>

<!-- table-layout fixed + larguras por coluna: editar uma célula não faz as
     outras colunas mudarem de tamanho. A última coluna, sem largura, preenche. -->
<table class="w-max min-w-full table-fixed border-separate border-spacing-0 text-xs">
  <colgroup>
    {#if editable}<col style:width="76px" />{/if}
    {#each columns as { column, width } (column.name)}<col style:width={`${width}px`} />{/each}
    <col />
  </colgroup>
  <thead class="sticky top-0 z-10">
    <tr>
      {#if editable}
        <th class={[stickyCell, 'z-[2] bg-card px-3 py-2 text-left']}>
          <Checkbox
            checked={selected.size > 0 && selected.size === data.rows.length}
            indeterminate={selected.size > 0 && selected.size < data.rows.length}
            onCheckedChange={(v) => toggleAll(v === true)}
            aria-label="Selecionar todas as linhas da página"
          />
        </th>
      {/if}
      {#each columns as { column, kind } (column.name)}
        <th class="border-r border-b bg-card p-0 align-top font-normal">
          <GridColumnHeader {column} {kind} sort={sortOf(column.name)} onsort={() => onsort(column.name)} />
        </th>
      {/each}
      <th class="border-b bg-card" aria-hidden="true"></th>
    </tr>
  </thead>
  <tbody>
    {#each data.rows as row, i (i)}
      {@const isSelected = selected.has(i)}
      <tr class={['group', isSelected ? 'bg-brand/5' : 'hover:bg-muted/50']}>
        {#if editable}
          <td class={[stickyCell, 'px-3 py-1.5', isSelected ? 'bg-[color-mix(in_oklch,var(--brand)_5%,var(--background))]' : 'group-hover:bg-[color-mix(in_oklch,var(--muted)_50%,var(--background))]']}>
            <div class="flex items-center gap-1.5">
              <Checkbox checked={isSelected} onCheckedChange={(v) => toggleRow(i, v === true)} aria-label={`Selecionar linha ${i + 1}`} />
              <button
                type="button"
                class="grid size-6 place-items-center rounded text-muted-foreground transition-opacity hover:bg-accent hover:text-foreground focus-visible:opacity-100 md:opacity-0 md:group-hover:opacity-100"
                aria-label={`Expandir linha ${i + 1}`}
                title="Ver e editar a linha inteira"
                onclick={() => onexpand(row)}
              >
                <Maximize2 class="size-3.5" />
              </button>
            </div>
          </td>
        {/if}
        {#each columns as { column, kind } (column.name)}
          {@const value = row[column.name]}
          <td
            class={['group/cell border-r border-b p-0', editable && !column.generated && 'cursor-text']}
            ondblclick={() => startEdit(i, column)}
          >
            {#if editing?.row === i && editing.column === column.name}
              <div class="flex items-center gap-1 bg-background p-0.5 ring-2 ring-brand ring-inset">
                <input
                  id="inline-editor"
                  class={[
                    'w-full min-w-0 bg-transparent px-2 py-1 text-xs outline-none',
                    monospace(kind) && 'font-mono',
                    alignRight(kind) && 'text-right',
                  ]}
                  bind:value={draft}
                  onkeydown={onEditorKey}
                  onblur={() => (editing = null)}
                  aria-label={`Editar ${column.name}`}
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
                <span class="min-w-0 flex-1"><GridCell {value} type={column.type} {kind} /></span>
                {#if column.references && value !== null}
                  <a
                    href={referenceHref(column, value)}
                    class="grid size-5 shrink-0 place-items-center rounded text-muted-foreground opacity-0 group-hover/cell:opacity-100 hover:bg-accent hover:text-brand focus-visible:opacity-100"
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
        <td class="border-b" aria-hidden="true"></td>
      </tr>
    {/each}
  </tbody>
</table>
