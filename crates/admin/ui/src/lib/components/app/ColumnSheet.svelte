<script lang="ts">
  import * as Sheet from '$lib/components/ui/sheet'
  import { Button } from '$lib/components/ui/button'
  import { toast } from 'svelte-sonner'
  import ColumnFields from './ColumnFields.svelte'
  import SqlPreview from './SqlPreview.svelte'
  import { blankColumn, columnChanges, ddl, toColumnDef, type AlterAction, type ColumnDef, type ColumnInfo } from '$lib/ddl'
  import { loadSchemaColumns, loadTypes } from '$lib/pg-types.svelte'
  import { SqlPreview as Preview } from '$lib/preview.svelte'

  let {
    open = $bindable(false),
    table,
    original = null,
    onsaved,
  }: {
    open?: boolean
    table: string
    /** Coluna sendo editada; `null` = adicionar coluna nova. */
    original?: ColumnInfo | null
    onsaved: () => void
  } = $props()

  let column = $state<ColumnDef>(blankColumn())
  let tables = $state<Record<string, string[]>>({})
  let saving = $state(false)
  const preview = new Preview()

  $effect(() => {
    if (!open) return
    column = original ? toColumnDef(original) : blankColumn()
    loadTypes()
    loadSchemaColumns()
      .then((t) => (tables = t))
      .catch(() => (tables = {}))
  })

  const actions = $derived.by((): AlterAction[] => {
    if (!column.name.trim() || !column.data_type.trim()) return []
    return original ? columnChanges(original, column) : [{ action: 'add_column', column: $state.snapshot(column) }]
  })

  $effect(() => {
    const pending = $state.snapshot(actions)
    if (!open || pending.length === 0) return preview.clear()
    preview.schedule((signal) => ddl.alterTable(table, pending, true, { signal }))
  })

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    saving = true
    try {
      const result = await ddl.alterTable(table, $state.snapshot(actions))
      toast.success(result.message ?? 'Coluna salva')
      open = false
      onsaved()
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      saving = false
    }
  }
</script>

<Sheet.Root bind:open>
  <Sheet.Content class="flex w-full flex-col gap-0 p-0 data-[side=right]:sm:max-w-2xl">
    <Sheet.Header class="border-b px-6 pt-6 pb-5">
      <Sheet.Title>{original ? `Editar coluna ${original.name}` : 'Nova coluna'}</Sheet.Title>
      <Sheet.Description>
        em <code class="font-mono text-foreground">{table}</code>
        {#if original?.primary_key}· faz parte da chave primária{/if}
      </Sheet.Description>
    </Sheet.Header>

    <form id="column-form" class="flex-1 space-y-6 overflow-y-auto px-6 py-6" onsubmit={submit}>
      <ColumnFields bind:column {tables} mode={original ? 'edit' : 'add'} />
      {#if original && column.data_type.trim() !== original.data_type}
        <p class="rounded-lg border bg-muted/40 px-4 py-3 text-xs text-muted-foreground">
          Os valores atuais são convertidos com <code>{original.name}::{column.data_type}</code>. Se algum não converter, nada é
          alterado.
        </p>
      {/if}
      <SqlPreview {preview} placeholder={original ? 'Nenhuma alteração ainda.' : 'Dê nome e tipo à coluna.'} />
    </form>

    <Sheet.Footer class="flex-row justify-end gap-2 border-t bg-muted/40 px-6 py-4">
      <Button variant="outline" onclick={() => (open = false)}>Cancelar</Button>
      <Button type="submit" form="column-form" disabled={saving || actions.length === 0}>
        {saving ? 'Salvando…' : original ? 'Salvar alterações' : 'Adicionar coluna'}
      </Button>
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>
