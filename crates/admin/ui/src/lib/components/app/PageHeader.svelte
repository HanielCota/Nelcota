<script lang="ts">
  import type { Component, Snippet } from 'svelte'

  let {
    title,
    description,
    icon: Icon,
    actions,
  }: { title: string; description?: string; icon?: Component; actions?: Snippet } = $props()
</script>

<div class="mb-8 flex flex-wrap items-center justify-between gap-x-6 gap-y-4 border-b pb-6 sm:mb-10">
  <div class="flex min-w-0 items-center gap-4">
    {#if Icon}
      <span
        class="hidden size-12 shrink-0 place-items-center rounded-xl border border-brand/20 bg-brand-soft text-brand shadow-card sm:grid"
        aria-hidden="true"
      >
        <Icon class="size-6" strokeWidth={1.75} />
      </span>
    {/if}
    <div class="grid min-w-0 gap-1">
      <h1 class="text-2xl font-bold tracking-tight sm:text-3xl">{title}</h1>
      {#if description}<p class="text-sm text-muted-foreground sm:text-base">{description}</p>{/if}
    </div>
  </div>
  {#if actions}
    <div class="flex flex-wrap items-center gap-2">{@render actions()}</div>
  {/if}
</div>
