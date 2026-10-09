<script lang="ts">
  import type { Snippet } from 'svelte'

  let {
    title,
    description,
    actions,
  }: { title: string; description?: string; actions?: Snippet } = $props()
</script>

<div class="mb-8 flex flex-wrap items-end justify-between gap-x-6 gap-y-4">
  <div class="grid min-w-0 gap-2">
    <h1 class="text-3xl font-semibold tracking-tight sm:text-4xl">{title}</h1>
    {#if description}<p class="text-sm text-muted-foreground">{description}</p>{/if}
  </div>
  {#if actions}
    <div class="flex flex-wrap items-center gap-2">{@render actions()}</div>
  {/if}
</div>
