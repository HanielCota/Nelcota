<script lang="ts">
  import { formatCell } from '$lib/format'
  import { alignRight, monospace, type ColumnKind } from '$lib/grid'

  let { value, type, kind }: { value: string | null; type: string; kind: ColumnKind } = $props()

  const display = $derived(value === null ? null : formatCell(value, type))
  /** JSON numa linha só: a célula mostra o começo; o valor inteiro fica no tooltip. */
  const compactJson = (text: string) => {
    try {
      return JSON.stringify(JSON.parse(text))
    } catch {
      return text
    }
  }
</script>

<!-- Só exibição; a edição e o tooltip usam o valor exato. -->
<span
  class={[
    'block truncate',
    alignRight(kind) && 'text-right tabular-nums',
    monospace(kind) && 'font-mono text-[0.95em]',
    kind === 'temporal' && 'tabular-nums',
  ]}
>
  {#if value === null}
    <span class="font-mono text-3xs text-muted-foreground/80">NULL</span>
  {:else if kind === 'boolean'}
    <span class={['font-mono', value === 'true' ? 'text-foreground' : 'text-muted-foreground']}>{value}</span>
  {:else if kind === 'enum'}
    <span class="rounded border px-1.5 py-px text-2xs">{value}</span>
  {:else if kind === 'json'}
    <span class="text-muted-foreground">{compactJson(value)}</span>
  {:else}
    {display?.text}
  {/if}
</span>
