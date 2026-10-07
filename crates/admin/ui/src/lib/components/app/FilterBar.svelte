<script lang="ts">
  import { untrack } from 'svelte'
  import * as Select from '$lib/components/ui/select'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import Plus from '@lucide/svelte/icons/plus'
  import X from '@lucide/svelte/icons/x'
  import { UI_OPERATORS, fromUi, needsValue, toUi, type TableFilter, type UiOp } from '$lib/filters'
  import type { Column } from '$lib/types'
  import { t } from '$lib/i18n/index.svelte'

  let {
    columns,
    filters,
    preset,
    onapply,
    onclose,
  }: {
    columns: Column[]
    filters: TableFilter[]
    /** Column for a new, prefilled row (column menu). */
    preset?: string
    onapply: (filters: TableFilter[]) => void
    onclose: () => void
  } = $props()

  type Row = { key: number; column: string; op: UiOp; value: string }

  let nextKey = 0
  const toRow = (f: TableFilter): Row => ({ key: nextKey++, column: f.column, ...toUi(f) })
  const blankRow = (column = columns[0]?.name ?? ''): Row => ({ key: nextKey++, column, op: 'eq', value: '' })

  // Local draft, copied once on open: it only reaches the URL (and the API)
  // when applied.
  let rows = $state<Row[]>(
    untrack(() =>
      preset ? [...filters.map(toRow), blankRow(preset)] : filters.length ? filters.map(toRow) : [blankRow()],
    ),
  )

  const opLabel = (op: UiOp) => {
    const key = UI_OPERATORS.find((o) => o.value === op)?.label
    return key ? t(key) : op
  }
  const complete = (r: Row) => r.column !== '' && (!needsValue(r.op) || r.value !== '')

  function apply(event: SubmitEvent) {
    event.preventDefault()
    onapply(rows.filter(complete).map((r) => fromUi(r.column, r.op, r.value)))
  }
</script>

<form class="grid gap-2.5 border-b bg-muted/40 px-4 py-4" onsubmit={apply}>
  {#each rows as row, i (row.key)}
    <div class="flex flex-wrap items-center gap-2">
      <span class="w-12 text-right text-xs text-muted-foreground">{i === 0 ? t('tables.filters.where') : t('tables.filters.and')}</span>
      <Select.Root type="single" bind:value={row.column}>
        <Select.Trigger class="w-48 font-mono text-xs">{row.column || t('tables.filters.column')}</Select.Trigger>
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
            <Select.Item value={op.value}>{t(op.label)}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
      {#if needsValue(row.op)}
        <Input bind:value={row.value} placeholder={t('tables.filters.value')} class="w-56 font-mono text-xs" />
      {/if}
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={t('tables.filters.remove')}
        onclick={() => (rows = rows.filter((r) => r.key !== row.key))}
      >
        <X />
      </Button>
    </div>
  {/each}

  <div class="mt-1 flex flex-wrap items-center gap-2 pl-14">
    <Button variant="outline" size="sm" onclick={() => (rows = [...rows, blankRow()])}>
      <Plus />{t('tables.filters.add')}
    </Button>
    <div class="ml-auto flex items-center gap-2">
      {#if filters.length}
        <Button variant="ghost" size="sm" onclick={() => onapply([])}>{t('tables.filters.clear')}</Button>
      {/if}
      <Button variant="ghost" size="sm" onclick={onclose}>{t('common.close')}</Button>
      <Button type="submit" size="sm">{t('tables.filters.apply')}</Button>
    </div>
  </div>
</form>
