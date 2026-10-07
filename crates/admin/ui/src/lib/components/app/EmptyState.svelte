<script lang="ts">
  import type { Component, Snippet } from 'svelte'

  // Estado vazio padrão do painel: ícone em destaque, título, texto e ações.
  let {
    icon: Icon,
    title,
    description,
    children,
    actions,
    class: className = '',
  }: {
    icon?: Component
    title: string
    description?: string
    /** Texto rico no lugar de `description` (links, código). */
    children?: Snippet
    actions?: Snippet
    class?: string
  } = $props()
</script>

<div class={['grid place-items-center rounded-xl border border-dashed bg-card/40 px-6 py-14 text-center', className]}>
  {#if Icon}
    <span class="grid size-14 place-items-center rounded-2xl border bg-card text-muted-foreground shadow-card">
      <Icon class="size-6" strokeWidth={1.5} />
    </span>
  {/if}
  <p class="mt-4 text-base font-semibold">{title}</p>
  {#if description || children}
    <p class="mt-1.5 max-w-md text-sm text-muted-foreground">
      {#if children}{@render children()}{:else}{description}{/if}
    </p>
  {/if}
  {#if actions}<div class="mt-6 flex flex-wrap justify-center gap-2">{@render actions()}</div>{/if}
</div>
