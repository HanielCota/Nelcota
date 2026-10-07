<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import X from '@lucide/svelte/icons/x'
  import ArrowUp from '@lucide/svelte/icons/arrow-up'
  import ArrowDown from '@lucide/svelte/icons/arrow-down'
  import { describe, type TableFilter } from '$lib/filters'

  let {
    filters,
    sort,
    onchange,
    onclearsort,
  }: {
    filters: TableFilter[]
    sort: { column: string; desc: boolean } | null
    onchange: (filters: TableFilter[]) => void
    onclearsort: () => void
  } = $props()

  const chip = 'inline-flex h-6 items-center gap-1 rounded-md border border-border-strong bg-muted pr-0.5 pl-2 text-2xs'
  const remove = 'grid size-5 place-items-center rounded text-muted-foreground hover:bg-accent hover:text-foreground'
</script>

<!-- O que está moldando a grade (ordem e filtros), cada um removível. -->
<div class="flex shrink-0 flex-wrap items-center gap-1.5 border-b px-4 py-2">
  {#if sort}
    <span class={chip}>
      {#if sort.desc}<ArrowDown class="size-3 text-brand" />{:else}<ArrowUp class="size-3 text-brand" />{/if}
      ordenado por <span class="font-mono">{sort.column}</span>
      <button class={remove} aria-label="Remover ordenação" onclick={onclearsort}><X class="size-3" /></button>
    </span>
  {/if}
  {#each filters as filter, i (i)}
    <span class={[chip, 'font-mono']}>
      {describe(filter)}
      <button class={remove} aria-label="Remover filtro" onclick={() => onchange(filters.filter((_, j) => j !== i))}
        ><X class="size-3" /></button
      >
    </span>
  {/each}
  {#if filters.length}
    <Button variant="ghost" size="xs" class="text-muted-foreground" onclick={() => onchange([])}>Limpar filtros</Button>
  {/if}
</div>
