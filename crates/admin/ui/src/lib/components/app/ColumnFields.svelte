<script lang="ts">
  import { untrack } from 'svelte'
  import * as Select from '$lib/components/ui/select'
  import { Input } from '$lib/components/ui/input'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import { Button } from '$lib/components/ui/button'
  import X from '@lucide/svelte/icons/x'
  import ChevronDown from '@lucide/svelte/icons/chevron-down'
  import { ON_DELETE, type ColumnDef, type OnDelete } from '$lib/ddl'
  import { pgTypes } from '$lib/pg-types.svelte'

  let {
    column = $bindable(),
    tables,
    mode,
    onremove,
  }: {
    column: ColumnDef
    /** Tabelas e colunas do schema, para a chave estrangeira. */
    tables: Record<string, string[]>
    /** `create`: dentro de "Nova tabela"; `add`/`edit`: aba Estrutura. */
    mode: 'create' | 'add' | 'edit'
    onremove?: () => void
  } = $props()

  const id = $props.id()
  // Começa recolhido só na criação de tabela (muitas colunas na tela).
  let expanded = $state(untrack(() => mode !== 'create'))

  const INTEGER = ['smallint', 'integer', 'bigint']
  const isInteger = $derived(INTEGER.includes(column.data_type.trim().toLowerCase()))
  // Na edição, PK e identity vêm da criação e não mudam por aqui.
  const structural = $derived(mode === 'edit')

  /** Sugestões de DEFAULT conforme o tipo. */
  const defaults = $derived.by(() => {
    const type = column.data_type.toLowerCase()
    if (type.startsWith('timestamp')) return ['now()']
    if (type === 'date') return ['current_date']
    if (type === 'uuid') return ['gen_random_uuid()']
    if (type === 'boolean') return ['false', 'true']
    if (type.startsWith('json')) return ["'{}'::jsonb", "'[]'::jsonb"]
    if (INTEGER.includes(type) || type.startsWith('numeric')) return ['0']
    return ["''"]
  })

  function setReference(table: string) {
    if (!table) {
      column.references = null
      return
    }
    const columns = tables[table] ?? []
    column.references = {
      table,
      column: columns.includes('id') ? 'id' : (columns[0] ?? ''),
      on_delete: column.references?.on_delete ?? 'no_action',
    }
  }

  const onDeleteLabel = (value: OnDelete) => ON_DELETE.find((o) => o.value === value)?.label ?? value
</script>

<div class="rounded-lg border bg-card">
  <div class="flex flex-wrap items-center gap-2.5 p-3">
    <Input bind:value={column.name} placeholder="nome" class="w-40 font-mono text-xs" aria-label="Nome da coluna" />
    <Input
      bind:value={column.data_type}
      list={`${id}-types`}
      placeholder="tipo"
      class="w-40 font-mono text-xs"
      aria-label="Tipo"
    />
    <datalist id={`${id}-types`}>
      {#each pgTypes.base as type (type)}<option value={type}></option>{/each}
      {#each pgTypes.enums as type (type)}<option value={type}>enum</option>{/each}
    </datalist>
    <Input
      bind:value={() => column.default ?? '', (v) => (column.default = v || null)}
      list={`${id}-defaults`}
      placeholder={column.identity ? 'identity' : 'default (SQL)'}
      disabled={column.identity}
      class="min-w-32 flex-1 font-mono text-xs"
      aria-label="Valor padrão"
    />
    <datalist id={`${id}-defaults`}>
      {#each defaults as value (value)}<option {value}></option>{/each}
    </datalist>

    {#if mode === 'create'}
      <label class="flex cursor-pointer items-center gap-1.5 text-xs font-medium text-muted-foreground" title="Chave primária">
        <Checkbox bind:checked={column.primary_key} />PK
      </label>
    {/if}
    <label class="flex cursor-pointer items-center gap-1.5 text-xs font-medium text-muted-foreground" title="NOT NULL">
      <Checkbox
        checked={!column.nullable || column.primary_key}
        disabled={column.primary_key}
        onCheckedChange={(v) => (column.nullable = v !== true)}
      />obrigatória
    </label>

    <div class="ml-auto flex items-center">
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Mais opções"
        aria-expanded={expanded}
        onclick={() => (expanded = !expanded)}
      >
        <ChevronDown class={['transition-transform', expanded && 'rotate-180']} />
      </Button>
      {#if onremove}
        <Button variant="ghost" size="icon-sm" aria-label="Remover coluna" onclick={onremove}><X /></Button>
      {/if}
    </div>
  </div>

  {#if expanded}
    <div class="grid gap-4 rounded-b-xl border-t bg-muted/30 p-4 text-xs">
      <div class="flex flex-wrap gap-4">
        <label class="flex items-center gap-1.5 text-muted-foreground">
          <Checkbox bind:checked={column.unique} disabled={column.primary_key} />valor único (UNIQUE)
        </label>
        {#if !structural}
          <label
            class={['flex items-center gap-1.5 text-muted-foreground', !isInteger && 'opacity-50']}
            title="Só para smallint, integer e bigint"
          >
            <Checkbox
              checked={column.identity}
              disabled={!isInteger}
              onCheckedChange={(v) => {
                column.identity = v === true
                if (column.identity) column.default = null
              }}
            />numeração automática (identity)
          </label>
        {/if}
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <span class="w-24 text-muted-foreground">Referencia</span>
        <Select.Root type="single" value={column.references?.table ?? ''} onValueChange={setReference}>
          <Select.Trigger size="sm" class="w-40 font-mono text-xs">
            {column.references?.table ?? 'nenhuma tabela'}
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="" class="text-xs">nenhuma tabela</Select.Item>
            {#each Object.keys(tables) as table (table)}
              <Select.Item value={table} class="font-mono text-xs">{table}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
        {#if column.references}
          <Select.Root type="single" bind:value={column.references.column}>
            <Select.Trigger size="sm" class="w-32 font-mono text-xs">{column.references.column}</Select.Trigger>
            <Select.Content>
              {#each tables[column.references.table] ?? [] as name (name)}
                <Select.Item value={name} class="font-mono text-xs">{name}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
          <span class="text-muted-foreground">ao apagar:</span>
          <Select.Root type="single" bind:value={column.references.on_delete}>
            <Select.Trigger size="sm" class="w-40 text-xs">{onDeleteLabel(column.references.on_delete)}</Select.Trigger>
            <Select.Content>
              {#each ON_DELETE as option (option.value)}
                <Select.Item value={option.value} class="text-xs">{option.label}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
        {/if}
      </div>

      <div class="flex items-center gap-2">
        <span class="w-24 text-muted-foreground">Descrição</span>
        <Input
          bind:value={() => column.comment ?? '', (v) => (column.comment = v || null)}
          placeholder="opcional (vira COMMENT e aparece na documentação da API)"
          class="flex-1 text-xs"
        />
      </div>
    </div>
  {/if}
</div>
