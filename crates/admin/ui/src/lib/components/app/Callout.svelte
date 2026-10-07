<script lang="ts">
  import type { Snippet } from 'svelte'

  // Block notice for alerts that need attention (security, data loss).
  let {
    variant = 'warning',
    title,
    children,
    class: className = '',
  }: {
    variant?: 'warning' | 'danger'
    title?: string
    children?: Snippet
    class?: string
  } = $props()
</script>

<div
  role="alert"
  class={[
    'rounded-lg border px-4 py-3 text-sm',
    variant === 'danger' ? 'border-destructive/30 bg-destructive/5' : 'border-warning/30 bg-warning/5',
    className,
  ]}
>
  {#if title}
    <p class={['font-medium', variant === 'danger' ? 'text-destructive' : 'text-warning']}>{title}</p>
  {/if}
  {#if children}
    <div class={[title ? 'mt-1 text-muted-foreground' : variant === 'danger' ? 'text-destructive' : 'text-warning']}>
      {@render children()}
    </div>
  {/if}
</div>
