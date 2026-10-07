<script lang="ts">
  import type { Snippet } from 'svelte'
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert'
  import CircleAlert from '@lucide/svelte/icons/circle-alert'
  import Info from '@lucide/svelte/icons/info'

  // Aviso em bloco: erro de carga, alerta de segurança ou dica.
  let {
    variant = 'info',
    title,
    children,
    class: className = '',
  }: {
    variant?: 'info' | 'warning' | 'danger'
    title?: string
    children?: Snippet
    class?: string
  } = $props()

  const styles = {
    info: 'border-border-strong bg-muted/50 [&_svg]:text-muted-foreground',
    warning: 'border-warning/30 bg-warning/5 [&_svg]:text-warning',
    danger: 'border-destructive/30 bg-destructive/5 [&_svg]:text-destructive',
  }
  const Icon = $derived(variant === 'danger' ? CircleAlert : variant === 'warning' ? TriangleAlert : Info)
</script>

<div
  role={variant === 'info' ? 'note' : 'alert'}
  class={['flex gap-3 rounded-xl border px-4 py-3.5 text-sm', styles[variant], className]}
>
  <Icon class="mt-0.5 size-[18px] shrink-0" />
  <div class="min-w-0">
    {#if title}
      <p class={['font-semibold', variant === 'danger' ? 'text-destructive' : variant === 'warning' ? 'text-warning' : '']}>
        {title}
      </p>
    {/if}
    {#if children}
      <div class={[title && 'mt-1', title ? 'text-muted-foreground' : variant === 'danger' && 'text-destructive']}>
        {@render children()}
      </div>
    {/if}
  </div>
</div>
