<script lang="ts">
  import type { Snippet } from 'svelte'

  // Estado vazio: uma frase curta e, se houver, a ação óbvia.
  let {
    title,
    description,
    children,
    actions,
    class: className = '',
  }: {
    title: string
    description?: string
    /** Texto rico no lugar de `description` (links, código). */
    children?: Snippet
    actions?: Snippet
    class?: string
  } = $props()
</script>

<div class={['px-6 py-12 text-center', className]}>
  <p class="text-sm font-medium">{title}</p>
  {#if description || children}
    <p class="mx-auto mt-1 max-w-md text-sm text-muted-foreground">
      {#if children}{@render children()}{:else}{description}{/if}
    </p>
  {/if}
  {#if actions}<div class="mt-4 flex flex-wrap justify-center gap-2">{@render actions()}</div>{/if}
</div>
