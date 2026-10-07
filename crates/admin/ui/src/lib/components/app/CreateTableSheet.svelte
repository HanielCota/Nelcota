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
    <Sheet.Header class="border-b px-6 pt-6 pb-5">
      <Sheet.Title>Nova tabela</Sheet.Title>
      <Sheet.Description>Criada no schema exposto pela API, numa transação só.</Sheet.Description>
    </Sheet.Header>

    <form id="create-table" class="flex-1 space-y-8 overflow-y-auto px-6 py-6" onsubmit={submit}>
      <div class="grid gap-4 sm:grid-cols-2">
        <div class="grid gap-2">
          <Label for="table-name" class="font-semibold">Nome</Label>
          <Input id="table-name" bind:value={spec.name} placeholder="ex.: pedidos" class="font-mono" required />
        </div>
        <div class="grid gap-2">
          <Label for="table-comment" class="font-semibold">Descrição</Label>
          <Input
            id="table-comment"
            bind:value={() => spec.comment ?? '', (v) => (spec.comment = v || null)}
            placeholder="opcional"
          />
        </div>
      </div>

      <label class="flex cursor-pointer items-start gap-3 rounded-xl border bg-card p-4 shadow-card transition-colors hover:border-border-strong has-data-checked:border-brand/40 has-data-checked:bg-brand-soft/50">
        <Checkbox bind:checked={spec.rls} class="mt-0.5" />
        <span class="text-sm font-semibold">
          Ativar Row Level Security (recomendado)
          <span class="mt-1 block text-xs font-normal text-muted-foreground">
            Sem policies, só o <code>service_role</code> acessa as linhas. Crie policies depois, na página Policies.
          </span>
        </span>
      </label>
      {#if !spec.rls}
        <p class="flex gap-2.5 rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive">
          <ShieldAlert class="size-4 shrink-0" />
          Sem RLS, quem tiver GRANT na tabela lê e altera todas as linhas.
        </p>
      {/if}

      <section class="grid gap-3">
        <h3 class="text-base font-semibold">Colunas</h3>
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

      <section class="grid gap-3">
        <h3 class="text-base font-semibold">Acesso pela API (GRANT)</h3>
        <GrantsEditor bind:grants={spec.grants} />
      </section>

      <SqlPreview {preview} placeholder="Dê um nome à tabela e às colunas para ver o SQL." />
    </form>

    <Sheet.Footer class="flex-row justify-end gap-2 border-t bg-muted/40 px-6 py-4">
      <Button variant="outline" onclick={() => (open = false)}>Cancelar</Button>
      <Button type="submit" form="create-table" disabled={saving || !ready}>
        {saving ? 'Criando…' : 'Criar tabela'}
      </Button>
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>
