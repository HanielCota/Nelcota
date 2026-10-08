<script lang="ts">
  import type { Snippet } from 'svelte'
  import Inbox from '@lucide/svelte/icons/inbox'

  // Empty state: a short sentence and, if there is one, the obvious action.
  let {
    title,
    description,
    children,
    actions,
    class: className = '',
    icon: Icon = Inbox,
  }: {
    title: string
    description?: string
    /** Rich text instead of `description` (links, code). */
    children?: Snippet
    actions?: Snippet
    class?: string
    icon?: typeof Inbox
  } = $props()
</script>

<div class={['px-6 py-12 text-center', className]}>
  <div class="mx-auto mb-4 grid size-12 place-items-center rounded-xl border bg-muted/40 text-muted-foreground"><Icon class="size-6" aria-hidden="true" /></div>
  <p class="text-sm font-medium">{title}</p>
  {#if description || children}
    <p class="mx-auto mt-1 max-w-md text-sm text-muted-foreground">
      {#if children}{@render children()}{:else}{description}{/if}
    </p>
  {/if}
  {#if actions}<div class="mt-4 flex flex-wrap justify-center gap-2">{@render actions()}</div>{/if}
</div>
