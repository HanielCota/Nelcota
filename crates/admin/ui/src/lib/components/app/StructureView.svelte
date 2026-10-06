<script lang="ts">
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { toast } from 'svelte-sonner'
  import StructureColumns from './StructureColumns.svelte'
  import TableSettings from './TableSettings.svelte'
  import ColumnSheet from './ColumnSheet.svelte'
  import ConfirmDialog from './ConfirmDialog.svelte'
  import DropTableDialog from './DropTableDialog.svelte'
  import { ddl, type AlterAction, type ColumnInfo, type Structure } from '$lib/ddl'

  let {
    name,
    onrenamed,
    ondropped,
  }: {
    name: string
    /** A tabela mudou de nome: o editor troca a URL. */
    onrenamed: (name: string) => void
    ondropped: () => void
  } = $props()

  let structure = $state<Structure | null>(null)
  let error = $state('')
  let columnOpen = $state(false)
  let editing = $state<ColumnInfo | null>(null)
  let toDelete = $state<ColumnInfo | null>(null)
  let deleteOpen = $state(false)
  let dropOpen = $state(false)

  async function load() {
    try {
      structure = await ddl.structure(name)
      error = ''
    } catch (e) {
      error = (e as Error).message
    }
  }

  $effect(() => {
    void name
    load()
  })

  /** Ponto único de alteração: aplica, avisa e recarrega (ou segue o rename). */
  async function alter(actions: AlterAction[]) {
    if (actions.length === 0) return
    try {
      const result = await ddl.alterTable(name, actions)
      toast.success(result.message ?? 'Tabela alterada')
      const renamed = actions.findLast((a) => a.action === 'rename_table')
      if (renamed) onrenamed(renamed.name)
      else await load()
    } catch (e) {
      toast.error((e as Error).message)
      throw e
    }
  }

  function openColumn(column: ColumnInfo | null) {
    editing = column
    columnOpen = true
  }
</script>

{#if error}
  <p class="p-6 text-sm text-destructive">{error}</p>
{:else if !structure}
  <div class="grid gap-4 p-6"><Skeleton class="h-64 rounded-lg" /><Skeleton class="h-32 rounded-lg" /></div>
{:else}
  <div class="mx-auto grid max-w-5xl gap-6 p-6">
    <StructureColumns
      {structure}
      onadd={() => openColumn(null)}
      onedit={openColumn}
      ondelete={(column) => {
        toDelete = column
        deleteOpen = true
      }}
    />
    <TableSettings {structure} onalter={(actions) => alter(actions).catch(() => {})} ondrop={() => (dropOpen = true)} />
  </div>

  <ColumnSheet bind:open={columnOpen} table={name} original={editing} onsaved={load} />
  <DropTableDialog bind:open={dropOpen} table={name} {ondropped} />
  {#if toDelete}
    <ConfirmDialog
      bind:open={deleteOpen}
      title={`Apagar a coluna ${toDelete.name}?`}
      description="Os valores da coluna somem em todas as linhas. Não dá para desfazer."
      confirmLabel="Apagar coluna"
      destructive
      onconfirm={() => alter([{ action: 'drop_column', name: toDelete!.name }])}
    />
  {/if}
{/if}
