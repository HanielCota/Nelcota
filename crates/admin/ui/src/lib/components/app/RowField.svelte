<script lang="ts">
  import * as Select from '$lib/components/ui/select'
  import { Input } from '$lib/components/ui/input'
  import { Textarea } from '$lib/components/ui/textarea'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import { Button } from '$lib/components/ui/button'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import { formatCell } from '$lib/format'
  import { columnKind } from '$lib/grid'
  import type { FieldState } from '$lib/row-form'
  import type { Column } from '$lib/types'

  let {
    column,
    field = $bindable(),
    inserting,
    problem,
  }: {
    column: Column
    field: FieldState
    inserting: boolean
    problem: string | null
  } = $props()

  const id = $derived(`f-${column.name}`)
  const kind = $derived(columnKind(column))
  // Texto curto numa linha; vira área de texto se já for longo ou tiver quebra.
  const multiline = $derived(kind === 'json' || (kind === 'text' && (field.value.length > 80 || field.value.includes('\n'))))
  const placeholder = $derived(inserting && column.has_default ? 'DEFAULT' : column.nullable ? 'NULL' : '')
  const required = $derived(inserting && !column.nullable && !column.has_default)
  const preview = $derived(
    kind === 'temporal' && field.value && !field.isNull ? formatCell(field.value, column.type) : null,
  )
  const options = $derived(kind === 'boolean' ? ['true', 'false'] : column.enum_values)

  function formatJson() {
    try {
      field.value = JSON.stringify(JSON.parse(field.value), null, 2)
    } catch {
      // Inválido: a mensagem de erro já está visível.
    }
  }
</script>

<div class="grid gap-1.5">
  <div class="flex items-center gap-2">
    <label for={id} class="flex items-center gap-1.5 text-sm font-medium">
      {#if column.is_pk}<KeyRound class="size-3.5 text-brand" aria-label="chave primária" />{/if}{column.name}
    </label>
    <span class="font-mono text-3xs text-muted-foreground">{column.full_type}</span>
    {#if required}<span class="text-3xs text-muted-foreground">obrigatória</span>{/if}
    {#if column.nullable}
      <label class="ml-auto flex items-center gap-1.5 text-xs text-muted-foreground">
        <Checkbox checked={field.isNull} onCheckedChange={(v) => (field.isNull = v === true)} />
        NULL
      </label>
    {/if}
  </div>

  {#if field.isNull}
    <div class="flex h-9 items-center rounded-md border border-dashed px-3 font-mono text-xs text-muted-foreground">NULL</div>
  {:else if options.length}
    <Select.Root type="single" bind:value={field.value}>
      <Select.Trigger {id} class="w-full">{field.value || placeholder || 'Selecione'}</Select.Trigger>
      <Select.Content>
        {#each options as option (option)}<Select.Item value={option}>{option}</Select.Item>{/each}
      </Select.Content>
    </Select.Root>
  {:else if multiline}
    <Textarea
      {id}
      class={['min-h-20 text-xs', kind === 'json' && 'min-h-28 font-mono']}
      {placeholder}
      bind:value={field.value}
      aria-invalid={problem ? true : undefined}
    />
  {:else}
    <Input
      {id}
      class={['text-sm', (kind === 'number' || kind === 'uuid' || kind === 'temporal') && 'font-mono text-xs']}
      inputmode={kind === 'number' ? 'decimal' : undefined}
      {placeholder}
      bind:value={field.value}
      aria-invalid={problem ? true : undefined}
    />
  {/if}

  {#if problem}
    <p class="text-xs text-destructive">{problem}</p>
  {:else if preview && preview.text !== field.value}
    <p class="text-xs font-light text-muted-foreground">= {preview.text}{column.type === 'timestamp with time zone' ? ' no seu fuso' : ''}</p>
  {/if}
  {#if kind === 'json' && !field.isNull && field.value.trim()}
    <Button variant="ghost" size="xs" class="justify-self-start text-muted-foreground" onclick={formatJson}>Formatar JSON</Button>
  {/if}
  {#if column.comment}<p class="text-xs font-light text-muted-foreground">{column.comment}</p>{/if}
</div>
