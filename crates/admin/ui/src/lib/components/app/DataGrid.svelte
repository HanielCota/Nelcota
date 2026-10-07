<script lang="ts">
  import { tick } from 'svelte'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import Maximize2 from '@lucide/svelte/icons/maximize-2'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import { toast } from 'svelte-sonner'
  import GridCell from './GridCell.svelte'
  import GridColumnHeader from './GridColumnHeader.svelte'
  import { alignRight, columnKind, columnWidth, monospace, nextCell, type CellPos } from '$lib/grid'
  import type { Column, RowData, TableData } from '$lib/types'
  import { t } from '$lib/i18n/index.svelte'

  let {
    data,
    sort,
    hidden,
    selected = $bindable(),
    onsort,
    onsortset,
    onfilter,
    onhide,
    onexpand,
    oncommit,
    referenceHref,
  }: {
    data: TableData
    sort: { column: string; desc: boolean } | null
    /** Columns the user hid. */
    hidden: string[]
    selected: Set<number>
    onsort: (column: string) => void
    onsortset: (column: string, direction: 'asc' | 'desc' | null) => void
    onfilter: (column: string) => void
    onhide: (column: string) => void
    onexpand: (row: RowData) => void
    /** Saves a cell; resolves once stored (or rejects with the error already shown). */
    oncommit: (row: number, column: string, value: string | null) => Promise<void>
    referenceHref: (column: Column, value: string) => string
  } = $props()

  const columns = $derived(
    data.table.columns
      .filter((c) => !hidden.includes(c.name))
      .map((c) => ({ column: c, kind: columnKind(c), width: columnWidth(c) })),
  )
  const editable = $derived(data.table.editable)

  // Inline editing: the cell (row, column) being edited and the draft.
  let editing = $state<{ row: number; column: string } | null>(null)
  let draft = $state('')

  // Keyboard navigation (WAI-ARIA "grid" pattern): one active cell at a time
  // takes focus (roving tabindex); Tab enters and leaves the whole grid.
  let active = $state<CellPos>({ row: 0, col: 0 })
  let tableEl = $state<HTMLTableElement>()

  // Page, filter or columns changed: the active cell moves back within
  // bounds. Only assign on a real change: with 0 rows the clamped position is
  // the same, and assigning a new object would rerun this effect forever.
  $effect(() => {
    const row = Math.min(active.row, Math.max(data.rows.length - 1, 0))
    const col = Math.min(active.col, Math.max(columns.length - 1, 0))
    if (row !== active.row || col !== active.col) active = { row, col }
  })

  async function focusCell(pos: CellPos) {
    active = pos
    await tick()
    const cell = tableEl?.querySelector<HTMLElement>(`[data-cell="${pos.row}:${pos.col}"]`)
    cell?.focus({ preventScroll: true })
    cell?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }

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
    focusCell(active)
    if (value === data.rows[row][column]) return
    await oncommit(row, column, value).catch(() => {})
  }

  function cancelEdit() {
    editing = null
    focusCell(active)
  }

  function onEditorKey(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      commitEdit()
    } else if (event.key === 'Escape') {
      event.preventDefault()
      cancelEdit()
    }
  }

  async function copyCell(value: string | null) {
    await navigator.clipboard.writeText(value ?? '')
    toast.success(value === null ? t('tables.toast.nullCopied') : t('tables.toast.valueCopied'))
  }

  function onCellKey(event: KeyboardEvent, row: number, col: number) {
    if (editing) return
    const ctrl = event.ctrlKey || event.metaKey
    const next = nextCell(event.key, { row, col }, data.rows.length, columns.length, ctrl)
    if (next) {
      event.preventDefault()
      focusCell(next)
      return
    }
    const column = columns[col].column
    if (event.key === 'Enter' || event.key === 'F2') {
      event.preventDefault()
      startEdit(row, column)
    } else if (event.key === ' ' && editable) {
      event.preventDefault()
      toggleRow(row, !selected.has(row))
    } else if (event.key === 'Escape' && selected.size) {
      selected = new Set()
    } else if (ctrl && event.key.toLowerCase() === 'c' && !window.getSelection()?.toString()) {
      event.preventDefault()
      copyCell(data.rows[row][column.name])
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

  // Column pinned to the left (selection + expand): opaque background so the
  // content scrolling underneath does not show through.
  const stickyCell = 'sticky left-0 z-[1] border-r border-b bg-background'
</script>

<!-- table-layout fixed + per-column widths: editing a cell does not resize
     the other columns. The last column, without a width, fills the rest. -->
<table
  bind:this={tableEl}
  role="grid"
  aria-label={t('tables.grid.label', { table: data.table.name })}
  aria-multiselectable={editable}
  class="w-max min-w-full table-fixed border-separate border-spacing-0 text-xs"
>
  <colgroup>
    {#if editable}<col style:width="76px" />{/if}
    {#each columns as { column, width } (column.name)}<col style:width={`${width}px`} />{/each}
    <col />
  </colgroup>
  <thead class="sticky top-0 z-10">
    <tr>
      {#if editable}
        <th class={[stickyCell, 'z-[2] bg-card px-3.5 py-2.5 text-left shadow-[inset_0_-1px_0_var(--border)]']}>
          <Checkbox
            checked={selected.size > 0 && selected.size === data.rows.length}
            indeterminate={selected.size > 0 && selected.size < data.rows.length}
            onCheckedChange={(v) => toggleAll(v === true)}
            aria-label={t('tables.grid.selectAll')}
          />
        </th>
      {/if}
      {#each columns as { column, kind } (column.name)}
        <th class="border-r border-b bg-card p-0 align-top font-normal shadow-[inset_0_-1px_0_var(--border)]">
          <GridColumnHeader
            {column}
            {kind}
            sort={sortOf(column.name)}
            onsort={() => onsort(column.name)}
            onsortset={(direction) => onsortset(column.name, direction)}
            onfilter={() => onfilter(column.name)}
            onhide={() => onhide(column.name)}
          />
        </th>
      {/each}
      <th class="border-b bg-card" aria-hidden="true"></th>
    </tr>
  </thead>
  <tbody>
    {#each data.rows as row, i (i)}
      {@const isSelected = selected.has(i)}
      <tr class={['group transition-colors', isSelected ? 'bg-brand/[0.07]' : 'hover:bg-muted/60']}>
        {#if editable}
          <td role="gridcell" class={[stickyCell, 'px-3.5 py-2', isSelected ? 'bg-[color-mix(in_oklch,var(--brand)_7%,var(--background))]' : 'group-hover:bg-[color-mix(in_oklch,var(--muted)_60%,var(--background))]']}>
            <div class="flex items-center gap-2">
              <Checkbox checked={isSelected} onCheckedChange={(v) => toggleRow(i, v === true)} aria-label={t('tables.grid.selectRow', { n: i + 1 })} />
              <button
                type="button"
                class="grid size-7 cursor-pointer place-items-center rounded-md text-muted-foreground transition-opacity hover:bg-accent hover:text-foreground focus-visible:opacity-100 md:opacity-0 md:group-hover:opacity-100"
                aria-label={t('tables.grid.expandRow', { n: i + 1 })}
                title={t('tables.grid.expandTitle')}
                onclick={() => onexpand(row)}
              >
                <Maximize2 class="size-3.5" />
              </button>
            </div>
          </td>
        {/if}
        {#each columns as { column, kind }, c (column.name)}
          {@const value = row[column.name]}
          <td
            role="gridcell"
            data-cell={`${i}:${c}`}
            tabindex={active.row === i && active.col === c ? 0 : -1}
            aria-selected={isSelected}
            class={[
              'group/cell border-r border-b p-0 focus-visible:bg-brand/[0.06] focus-visible:outline-offset-[-2px]',
              editable && !column.generated && 'cursor-text',
            ]}
            onfocus={() => (active = { row: i, col: c })}
            onkeydown={(e) => onCellKey(e, i, c)}
            ondblclick={() => startEdit(i, column)}
          >
            {#if editing?.row === i && editing.column === column.name}
              <div class="flex items-center gap-1 bg-background p-0.5 ring-2 ring-brand ring-inset">
                <input
                  id="inline-editor"
                  class={[
                    'w-full min-w-0 bg-transparent px-2.5 py-1.5 text-xs outline-none',
                    monospace(kind) && 'font-mono',
                    alignRight(kind) && 'text-right',
                  ]}
                  bind:value={draft}
                  onkeydown={onEditorKey}
                  onblur={() => (editing = null)}
                  aria-label={t('tables.grid.edit', { column: column.name })}
                />
                {#if column.nullable}
                  <button
                    class="shrink-0 cursor-pointer rounded border border-border-strong bg-muted px-1.5 py-0.5 font-mono text-3xs text-muted-foreground hover:text-foreground"
                    onmousedown={(e) => {
                      e.preventDefault()
                      commitEdit(true)
                    }}>NULL</button
                  >
                {/if}
              </div>
            {:else}
              <div class="flex min-h-10 items-center gap-1 px-3.5 py-2" title={value ?? 'NULL'}>
                <span class="min-w-0 flex-1"><GridCell {value} type={column.type} {kind} /></span>
                {#if column.references && value !== null}
                  <a
                    href={referenceHref(column, value)}
                    class="grid size-6 shrink-0 place-items-center rounded-md text-muted-foreground opacity-0 group-hover/cell:opacity-100 hover:bg-accent hover:text-brand focus-visible:opacity-100"
                    title={t('tables.grid.openIn', { table: column.references.table })}
                    aria-label={t('tables.grid.openReferenced', { table: column.references.table })}
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
