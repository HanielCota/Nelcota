<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import X from '@lucide/svelte/icons/x'
  import { describe, type TableFilter } from '$lib/filters'

  let { filters, onchange }: { filters: TableFilter[]; onchange: (filters: TableFilter[]) => void } = $props()
</script>

<div class="flex shrink-0 flex-wrap items-center gap-1.5 border-b px-4 py-2">
  {#each filters as filter, i (i)}
    <span class="inline-flex h-6 items-center gap-1 rounded-md border border-border-strong bg-muted pr-0.5 pl-2 font-mono text-2xs">
      {describe(filter)}
      <button
        class="grid size-5 place-items-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
        aria-label="Remover filtro"
        onclick={() => onchange(filters.filter((_, j) => j !== i))}><X class="size-3" /></button
      >
    </span>
  {/each}
  <Button variant="ghost" size="xs" class="text-muted-foreground" onclick={() => onchange([])}>Limpar</Button>
</div>
