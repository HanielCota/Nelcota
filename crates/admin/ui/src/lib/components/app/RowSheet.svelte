<script lang="ts">
  import * as Sheet from '$lib/components/ui/sheet'
  import { Button } from '$lib/components/ui/button'
  import { toast } from 'svelte-sonner'
  import RowField from './RowField.svelte'
  import { api, enc } from '$lib/api'
  import { fieldProblem, initialFields, rowPayload, type FieldState } from '$lib/row-form'
  import type { Column, RowData } from '$lib/types'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  let {
    open = $bindable(false),
    table,
    columns,
    primaryKey,
    row = null,
    onsaved,
  }: {
    open?: boolean
    table: string
    columns: Column[]
    primaryKey: string[]
    /** `null` = insert a new row. */
    row?: RowData | null
    onsaved: () => void
  } = $props()

  let fields = $state<Record<string, FieldState>>({})
  let saving = $state(false)

  // Reload the fields whenever the sheet opens.
  $effect(() => {
    if (open) fields = initialFields(columns, row)
  })

  const inserting = $derived(row === null)
  const editable = $derived(columns.filter((c) => !c.generated))
  const generated = $derived(columns.filter((c) => c.generated))
  const problems = $derived(
    Object.fromEntries(editable.map((c) => [c.name, fields[c.name] ? fieldProblem(c, fields[c.name], inserting) : null])),
  )
  const valid = $derived(Object.values(problems).every((p) => p === null))
  const payload = $derived(rowPayload(columns, fields, row))
  const changes = $derived(Object.keys(payload).length)
  /** `id = 1` (or the composite key) to tell which row is open. */
  const pkLabel = $derived(row ? primaryKey.map((k) => `${k} = ${row![k] ?? 'NULL'}`).join(', ') : '')

  async function save(event?: SubmitEvent) {
    event?.preventDefault()
    if (!valid || saving) return
    if (row && changes === 0) {
      open = false
      return
    }
    saving = true
    try {
      const values = $state.snapshot(payload)
      if (row) {
        const res = await api.patch<{ count: number }>(`/tables/${enc(table)}/rows`, {
          pk: Object.fromEntries(primaryKey.map((k) => [k, row![k]])),
          values,
        })
        toast.success(t('tables.toast.rowsUpdated', { count: res.count }))
      } else {
        await api.post(`/tables/${enc(table)}/rows`, { values })
        toast.success(t('tables.toast.rowInserted'))
      }
      open = false
      onsaved()
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      saving = false
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
      event.preventDefault()
      save()
    }
  }
</script>

<Sheet.Root bind:open>
  <Sheet.Content class="flex w-full flex-col gap-0 p-0 data-[side=right]:sm:max-w-xl">
    <Sheet.Header class="border-b px-6 pt-6 pb-5">
      <Sheet.Title>{inserting ? t('tables.row.insertTitle') : t('tables.row.editTitle')}</Sheet.Title>
      <Sheet.Description>
        {t('tables.row.in')} <code class="font-mono text-foreground">{table}</code>{#if pkLabel}<span class="text-muted-foreground"
            >{` · `}</span
          ><code class="font-mono text-foreground">{pkLabel}</code>{/if}
      </Sheet.Description>
    </Sheet.Header>

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <form id="row-form" class="flex-1 space-y-6 overflow-y-auto px-6 py-6" onsubmit={save} onkeydown={onKeydown}>
      {#if generated.length}
        <div class="rounded-lg border bg-muted/40 px-4 py-3 text-xs">
          <p class="font-medium text-muted-foreground">{t('tables.row.generated')}</p>
          <dl class="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
            {#each generated as column (column.name)}
              <dt class="font-mono text-muted-foreground">{column.name}</dt>
              <dd class="truncate font-mono">{row ? (row[column.name] ?? 'NULL') : t('tables.row.setOnSave')}</dd>
            {/each}
          </dl>
        </div>
      {/if}
      {#each editable as column (column.name)}
        {#if fields[column.name]}
          <RowField {column} bind:field={fields[column.name]} {inserting} problem={problems[column.name]} />
        {/if}
      {/each}
    </form>

    <Sheet.Footer class="flex-row items-center gap-2 border-t bg-muted/40 px-6 py-4">
      <span class="mr-auto text-xs text-muted-foreground">
        {#if !inserting}{changes === 0 ? t('tables.row.noChanges') : t('tables.row.changed', { count: changes })}{/if}
        <span class="hidden sm:inline">{inserting ? '' : ' · '}{t('tables.row.saveHint')}</span>
      </span>
      <Button variant="outline" onclick={() => (open = false)}>{t('common.cancel')}</Button>
      <Button type="submit" form="row-form" disabled={saving || !valid || (!inserting && changes === 0)}>
        {saving ? t('common.saving') : inserting ? t('tables.row.insert') : t('common.save')}
      </Button>
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>
