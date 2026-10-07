<script lang="ts">
  import type { Snippet } from 'svelte'

  let {
    title,
    description,
    actions,
  }: { title: string; description?: string; actions?: Snippet } = $props()
</script>

<div class="mb-8 flex flex-wrap items-end justify-between gap-4">
  <div class="grid gap-1">
    <h1 class="text-2xl font-medium tracking-tight">{title}</h1>
    {#if description}<p class="text-sm font-light text-muted-foreground">{description}</p>{/if}
  </div>
  {#if actions}
    <div class="flex items-center gap-2">{@render actions()}</div>
  {/if}
</div>
