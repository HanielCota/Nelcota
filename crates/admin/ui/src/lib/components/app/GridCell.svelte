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
    <span class="rounded bg-muted px-1 py-px font-mono text-3xs text-muted-foreground">NULL</span>
  {:else if kind === 'boolean'}
    <span
      class={[
        'inline-flex h-5 items-center rounded-full px-2 font-mono text-2xs',
        value === 'true' ? 'bg-brand/10 text-brand dark:bg-brand/15' : 'bg-muted text-muted-foreground',
      ]}>{value}</span
    >
  {:else if kind === 'enum'}
    <span class="inline-flex h-5 items-center rounded-md border border-border-strong bg-muted/60 px-1.5 text-2xs">{value}</span>
  {:else if kind === 'json'}
    <span class="text-muted-foreground">{compactJson(value)}</span>
  {:else}
    {display?.text}
  {/if}
</span>
