<script lang="ts">
  import * as Sheet from '$lib/components/ui/sheet'
  import * as Select from '$lib/components/ui/select'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Textarea } from '$lib/components/ui/textarea'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import { toast } from 'svelte-sonner'
  import { api, enc } from '$lib/api'
  import type { Column, RowData } from '$lib/types'

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
    /** `null` = inserir linha nova. */
    row?: RowData | null
    onsaved: () => void
  } = $props()

  type Field = { value: string; isNull: boolean; touched: boolean }
  let fields = $state<Record<string, Field>>({})
  let saving = $state(false)

  const editable = $derived(columns.filter((c) => !c.generated))
  const isJson = (c: Column) => c.type === 'json' || c.type === 'jsonb'
  const isBool = (c: Column) => c.type === 'boolean'
  const isLong = (c: Column) => isJson(c) || c.type === 'text'

  // Recarrega os campos sempre que a sheet abre.
  $effect(() => {
    if (!open) return
    const next: Record<string, Field> = {}
    for (const c of columns) {
      const value = row ? row[c.name] : null
      next[c.name] = {
        value: value ?? '',
        isNull: row ? value === null : false,
        touched: false,
      }
    }
    fields = next
  })

  function placeholder(c: Column): string {
    if (!row && c.has_default) return 'DEFAULT'
    if (c.nullable) return 'NULL'
    return ''
  }

  async function save(event: SubmitEvent) {
    event.preventDefault()
    const values: Record<string, string | null> = {}
    for (const c of editable) {
      const f = fields[c.name]
      if (!f) continue
      if (row) {
        // Edição: só o que mudou.
        if (!f.touched) continue
        values[c.name] = f.isNull ? null : f.value
      } else {
        // Inserção: vazio fica de fora (vale o DEFAULT), salvo NULL explícito.
        if (f.isNull) values[c.name] = null
        else if (f.value !== '') values[c.name] = f.value
      }
    }
    if (row && Object.keys(values).length === 0) {
      open = false
      return
    }
    saving = true
    try {
      if (row) {
        const pk = Object.fromEntries(primaryKey.map((k) => [k, row![k]]))
        const res = await api.patch<{ message: string }>(`/tables/${enc(table)}/rows`, { pk, values })
        toast.success(res.message)
      } else {
        const res = await api.post<{ message: string }>(`/tables/${enc(table)}/rows`, { values })
        toast.success(res.message)
      }
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
  <Sheet.Content class="flex w-full flex-col gap-0 p-0 data-[side=right]:sm:max-w-xl">
    <Sheet.Header class="border-b px-6 py-4">
      <Sheet.Title>{row ? 'Editar linha' : 'Inserir linha'}</Sheet.Title>
      <Sheet.Description>
        em <code class="font-mono text-foreground">{table}</code>
      </Sheet.Description>
    </Sheet.Header>

    <form id="row-form" class="flex-1 space-y-5 overflow-y-auto px-6 py-5" onsubmit={save}>
      {#each editable as column (column.name)}
        {@const field = fields[column.name]}
        {#if field}
          <div class="grid gap-1.5">
            <div class="flex items-center gap-2">
              <label for={`f-${column.name}`} class="text-sm font-medium">{column.name}</label>
              <span class="font-mono text-[11px] text-muted-foreground"
                >{column.full_type}{column.is_pk ? ', pk' : ''}</span
              >
              {#if column.nullable}
                <label class="ml-auto flex items-center gap-1.5 text-xs text-muted-foreground">
                  <Checkbox
                    checked={field.isNull}
                    onCheckedChange={(v) => {
                      field.isNull = v === true
                      field.touched = true
                    }}
                  />
                  NULL
                </label>
              {/if}
            </div>

            {#if column.enum_values.length}
              <Select.Root
                type="single"
                value={field.value}
                disabled={field.isNull}
                onValueChange={(v) => {
                  field.value = v
                  field.touched = true
                }}
              >
                <Select.Trigger id={`f-${column.name}`} class="w-full">
                  {field.isNull ? 'NULL' : field.value || placeholder(column) || 'Selecione'}
                </Select.Trigger>
                <Select.Content>
                  {#each column.enum_values as option (option)}
                    <Select.Item value={option}>{option}</Select.Item>
                  {/each}
                </Select.Content>
              </Select.Root>
            {:else if isBool(column)}
              <Select.Root
                type="single"
                value={field.value}
                disabled={field.isNull}
                onValueChange={(v) => {
                  field.value = v
                  field.touched = true
                }}
              >
                <Select.Trigger id={`f-${column.name}`} class="w-full">
                  {field.isNull ? 'NULL' : field.value || placeholder(column) || 'Selecione'}
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="true">true</Select.Item>
                  <Select.Item value="false">false</Select.Item>
                </Select.Content>
              </Select.Root>
            {:else if isLong(column)}
              <Textarea
                id={`f-${column.name}`}
                class="min-h-20 font-mono text-xs"
                placeholder={placeholder(column)}
                disabled={field.isNull}
                bind:value={field.value}
                oninput={() => (field.touched = true)}
              />
            {:else}
              <Input
                id={`f-${column.name}`}
                class="font-mono text-xs"
                placeholder={placeholder(column)}
                disabled={field.isNull}
                bind:value={field.value}
                oninput={() => (field.touched = true)}
              />
            {/if}
            {#if column.comment}
              <p class="text-xs text-muted-foreground">{column.comment}</p>
            {/if}
          </div>
        {/if}
      {/each}
      {#if columns.some((c) => c.generated)}
        <p class="text-xs text-muted-foreground">
          {#each columns.filter((c) => c.generated) as c, i (c.name)}<code class="font-mono text-foreground"
              >{c.name}</code
            >{i < columns.filter((x) => x.generated).length - 1 ? ', ' : ''}{/each}: gerada(s) pelo Postgres
          (identity ou GENERATED ALWAYS), não aparece(m) no formulário.
        </p>
      {/if}
    </form>

    <Sheet.Footer class="flex-row justify-end gap-2 border-t px-6 py-4">
      <Button variant="outline" onclick={() => (open = false)}>Cancelar</Button>
      <Button type="submit" form="row-form" disabled={saving}>
        {saving ? 'Salvando…' : 'Salvar'}
      </Button>
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>
