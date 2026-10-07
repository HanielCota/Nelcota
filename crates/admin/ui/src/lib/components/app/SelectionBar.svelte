<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import Copy from '@lucide/svelte/icons/copy'
  import Download from '@lucide/svelte/icons/download'
  import X from '@lucide/svelte/icons/x'
  import { toast } from 'svelte-sonner'
  import { downloadText, toCsv, toJson } from '$lib/download'
  import type { RowData } from '$lib/types'

  let {
    table,
    columns,
    rows,
    deletable,
    onclear,
    ondelete,
  }: {
    table: string
    columns: string[]
    /** Linhas selecionadas, na ordem da grade. */
    rows: RowData[]
    deletable: boolean
    onclear: () => void
    ondelete: () => void
  } = $props()

  const matrix = $derived(rows.map((row) => columns.map((c) => row[c] ?? null)))
  const label = $derived(`${rows.length} ${rows.length === 1 ? 'linha selecionada' : 'linhas selecionadas'}`)

  async function copyJson() {
    await navigator.clipboard.writeText(toJson(columns, matrix))
    toast.success(`${label.replace('selecionada', 'copiada')} como JSON`)
  }

  function exportAs(format: 'csv' | 'json') {
    const name = `${table.replace(/[^\w-]+/g, '_')}-selecao.${format}`
    if (format === 'csv') downloadText(name, toCsv(columns, matrix), 'text/csv;charset=utf-8')
    else downloadText(name, toJson(columns, matrix), 'application/json')
  }
</script>

<!-- Ações sobre a seleção; aparece só com linhas marcadas. -->
<div
  class="flex min-h-12 shrink-0 flex-wrap items-center gap-2 border-b border-brand/20 bg-brand/5 px-4 py-2 text-sm"
  role="region"
  aria-label="Linhas selecionadas"
>
  <span class="font-medium text-foreground" aria-live="polite">{label}</span>
  <span class="text-muted-foreground">nesta página</span>
  <div class="ml-auto flex flex-wrap items-center gap-2">
    <Button variant="ghost" size="sm" onclick={copyJson}><Copy />Copiar JSON</Button>
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Button variant="ghost" size="sm" {...props}><Download />Exportar seleção</Button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-40">
        <DropdownMenu.Item onclick={() => exportAs('csv')}>CSV</DropdownMenu.Item>
        <DropdownMenu.Item onclick={() => exportAs('json')}>JSON</DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
    {#if deletable}
      <Button variant="destructive" size="sm" onclick={ondelete}><Trash2 />Apagar</Button>
    {/if}
    <Button variant="ghost" size="icon-sm" aria-label="Limpar seleção" title="Limpar seleção (Esc)" onclick={onclear}><X /></Button>
  </div>
</div>
