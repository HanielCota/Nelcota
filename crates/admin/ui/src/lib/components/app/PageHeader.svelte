<script lang="ts">
  import type { Snippet } from 'svelte'

  let { title, actions }: { title: string; actions?: Snippet } = $props()
</script>

<div class="mb-5 flex flex-wrap items-center justify-between gap-4">
  <h1 class="text-lg font-semibold">{title}</h1>
  {#if actions}
    <div class="flex items-center gap-2">{@render actions()}</div>
  {/if}
</div>
