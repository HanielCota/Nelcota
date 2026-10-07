<script lang="ts">
  import { untrack } from 'svelte'
  import * as Select from '$lib/components/ui/select'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import Plus from '@lucide/svelte/icons/plus'
  import X from '@lucide/svelte/icons/x'
  import { UI_OPERATORS, fromUi, needsValue, toUi, type TableFilter, type UiOp } from '$lib/filters'
  import type { Column } from '$lib/types'

  let {
    columns,
    filters,
    preset,
    onapply,
    onclose,
  }: {
    columns: Column[]
    filters: TableFilter[]
    /** Coluna para uma linha nova já preenchida (menu da coluna). */
    preset?: string
    onapply: (filters: TableFilter[]) => void
    onclose: () => void
  } = $props()

  type Row = { key: number; column: string; op: UiOp; value: string }

  let nextKey = 0
  const toRow = (f: TableFilter): Row => ({ key: nextKey++, column: f.column, ...toUi(f) })
  const blankRow = (column = columns[0]?.name ?? ''): Row => ({ key: nextKey++, column, op: 'eq', value: '' })

  // Rascunho local, copiado uma vez ao abrir: só vai para a URL (e para a
  // API) ao aplicar.
  let rows = $state<Row[]>(
    untrack(() =>
      preset ? [...filters.map(toRow), blankRow(preset)] : filters.length ? filters.map(toRow) : [blankRow()],
    ),
  )

  const opLabel = (op: UiOp) => UI_OPERATORS.find((o) => o.value === op)?.label ?? op
  const complete = (r: Row) => r.column !== '' && (!needsValue(r.op) || r.value !== '')

  function apply(event: SubmitEvent) {
    event.preventDefault()
    onapply(rows.filter(complete).map((r) => fromUi(r.column, r.op, r.value)))
  }
</script>

<form class="grid gap-2.5 border-b bg-muted/40 px-4 py-4" onsubmit={apply}>
  {#each rows as row, i (row.key)}
    <div class="flex flex-wrap items-center gap-2">
      <span class="w-12 text-right text-xs font-semibold text-muted-foreground uppercase">{i === 0 ? 'onde' : 'e'}</span>
      <Select.Root type="single" bind:value={row.column}>
        <Select.Trigger class="w-48 font-mono text-xs">{row.column || 'coluna'}</Select.Trigger>
        <Select.Content>
          {#each columns as column (column.name)}
            <Select.Item value={column.name} class="font-mono text-xs">{column.name}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
      <Select.Root type="single" bind:value={row.op}>
        <Select.Trigger class="w-44 text-sm">{opLabel(row.op)}</Select.Trigger>
        <Select.Content>
          {#each UI_OPERATORS as op (op.value)}
            <Select.Item value={op.value}>{op.label}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
      {#if needsValue(row.op)}
        <Input bind:value={row.value} placeholder="valor" class="w-56 font-mono text-xs" />
      {/if}
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Remover filtro"
        onclick={() => (rows = rows.filter((r) => r.key !== row.key))}
      >
        <X />
      </Button>
    </div>
  {/each}

  <div class="mt-1 flex flex-wrap items-center gap-2 pl-14">
    <Button variant="outline" size="sm" onclick={() => (rows = [...rows, blankRow()])}>
      <Plus />Adicionar filtro
    </Button>
    <div class="ml-auto flex items-center gap-2">
      {#if filters.length}
        <Button variant="ghost" size="sm" onclick={() => onapply([])}>Limpar</Button>
      {/if}
      <Button variant="ghost" size="sm" onclick={onclose}>Fechar</Button>
      <Button type="submit" size="sm">Aplicar</Button>
    </div>
  </div>
</form>
