<script lang="ts">
  import * as Sheet from '$lib/components/ui/sheet'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import Plus from '@lucide/svelte/icons/plus'
  import ShieldAlert from '@lucide/svelte/icons/shield-alert'
  import { toast } from 'svelte-sonner'
  import ColumnFields from './ColumnFields.svelte'
  import GrantsEditor from './GrantsEditor.svelte'
  import SqlPreview from './SqlPreview.svelte'
  import { blankColumn, ddl, type CreateTable } from '$lib/ddl'
  import { loadSchemaColumns, loadTypes } from '$lib/pg-types.svelte'
  import { SqlPreview as Preview } from '$lib/preview.svelte'

  let { open = $bindable(false), oncreated }: { open?: boolean; oncreated: (name: string) => void } = $props()

  // Padrões no estilo Supabase: id identity, criado_em, RLS ligado e
  // service_role com acesso total (o resto se concede depois, com policies).
  const initial = (): CreateTable => ({
    name: '',
    comment: null,
    rls: true,
    columns: [
      { ...blankColumn(), name: 'id', data_type: 'bigint', primary_key: true, identity: true, nullable: false },
      { ...blankColumn(), name: 'criado_em', data_type: 'timestamptz', default: 'now()', nullable: false },
    ],
    grants: [
      { role: 'anon', privileges: [] },
      { role: 'authenticated', privileges: [] },
      { role: 'service_role', privileges: ['select', 'insert', 'update', 'delete'] },
    ],
  })

  let spec = $state<CreateTable>(initial())
  let keys = $state<number[]>([0, 1])
  let nextKey = 2
  let tables = $state<Record<string, string[]>>({})
  let saving = $state(false)
  const preview = new Preview()

  $effect(() => {
    if (!open) return
    spec = initial()
    keys = [0, 1]
    loadTypes()
    loadSchemaColumns()
      .then((t) => (tables = t))
      .catch(() => (tables = {}))
  })

  const ready = $derived(spec.name.trim() !== '' && spec.columns.length > 0 && spec.columns.every((c) => c.name.trim()))

  // Prévia a cada mudança do formulário (o snapshot também registra a dependência).
  $effect(() => {
    const snapshot = $state.snapshot(spec)
    if (!open || !ready) return preview.clear()
    preview.schedule((signal) => ddl.createTable(snapshot, true, { signal }))
  })

  function addColumn() {
    spec.columns.push(blankColumn())
    keys.push(nextKey++)
  }

  function removeColumn(index: number) {
    spec.columns.splice(index, 1)
    keys.splice(index, 1)
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    saving = true
    try {
      const result = await ddl.createTable($state.snapshot(spec))
      toast.success(result.message ?? 'Tabela criada')
      open = false
      oncreated(spec.name.trim())
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      saving = false
    }
  }
</script>

<Sheet.Root bind:open>
  <Sheet.Content class="flex w-full flex-col gap-0 p-0 data-[side=right]:sm:max-w-3xl">
    <Sheet.Header class="border-b px-6 py-4">
      <Sheet.Title>Nova tabela</Sheet.Title>
      <Sheet.Description>Criada no schema exposto pela API, numa transação só.</Sheet.Description>
    </Sheet.Header>

    <form id="create-table" class="flex-1 space-y-6 overflow-y-auto px-6 py-5" onsubmit={submit}>
      <div class="grid gap-4 sm:grid-cols-2">
        <div class="grid gap-1.5">
          <Label for="table-name" class="font-normal text-muted-foreground">Nome</Label>
          <Input id="table-name" bind:value={spec.name} placeholder="ex.: pedidos" class="font-mono" required />
        </div>
        <div class="grid gap-1.5">
          <Label for="table-comment" class="font-normal text-muted-foreground">Descrição</Label>
          <Input
            id="table-comment"
            bind:value={() => spec.comment ?? '', (v) => (spec.comment = v || null)}
            placeholder="opcional"
          />
        </div>
      </div>

      <label class="flex items-start gap-3 rounded-lg border p-3">
        <Checkbox bind:checked={spec.rls} class="mt-0.5" />
        <span class="text-sm">
          Ativar Row Level Security (recomendado)
          <span class="mt-0.5 block text-xs font-light text-muted-foreground">
            Sem policies, só o <code>service_role</code> acessa as linhas. Crie policies depois, na página Policies.
          </span>
        </span>
      </label>
      {#if !spec.rls}
        <p class="flex gap-2 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive">
          <ShieldAlert class="size-4 shrink-0" />
          Sem RLS, quem tiver GRANT na tabela lê e altera todas as linhas.
        </p>
      {/if}

      <section class="grid gap-2">
        <h3 class="text-sm font-medium">Colunas</h3>
        {#each spec.columns as _, i (keys[i])}
          <ColumnFields
            bind:column={spec.columns[i]}
            {tables}
            mode="create"
            onremove={spec.columns.length > 1 ? () => removeColumn(i) : undefined}
          />
        {/each}
        <Button variant="outline" size="sm" class="justify-self-start" onclick={addColumn}><Plus />Adicionar coluna</Button>
      </section>

      <section class="grid gap-2">
        <h3 class="text-sm font-medium">Acesso pela API (GRANT)</h3>
        <GrantsEditor bind:grants={spec.grants} />
      </section>

      <SqlPreview {preview} placeholder="Dê um nome à tabela e às colunas para ver o SQL." />
    </form>

    <Sheet.Footer class="flex-row justify-end gap-2 border-t px-6 py-4">
      <Button variant="outline" onclick={() => (open = false)}>Cancelar</Button>
      <Button type="submit" form="create-table" disabled={saving || !ready}>
        {saving ? 'Criando…' : 'Criar tabela'}
      </Button>
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>
