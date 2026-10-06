<script lang="ts">
  import { cn } from '$lib/utils'
  import type { Rls } from '$lib/types'

  let { rls, class: className = '' }: { rls: Rls; class?: string } = $props()

  const styles: Record<string, string> = {
    ok: 'border-primary/30 bg-primary/10 text-primary',
    warn: 'border-warning/30 bg-warning/10 text-warning',
    danger: 'border-destructive/30 bg-destructive/10 text-destructive',
    none: 'border-border text-muted-foreground',
    view: 'border-border text-muted-foreground',
  }
  const dots: Record<string, string> = {
    ok: 'bg-primary',
    warn: 'bg-warning',
    danger: 'bg-destructive',
    none: 'bg-muted-foreground',
    view: 'bg-muted-foreground',
  }
</script>

<span
  class={cn(
    'inline-flex h-5 shrink-0 items-center gap-1.5 rounded-full border px-2 text-[11px] font-medium whitespace-nowrap',
    styles[rls.state],
    className,
  )}
>
  <span class={cn('size-1.5 rounded-full', dots[rls.state])}></span>
  {rls.state === 'danger' ? '⚠ ' : ''}{rls.label}
</span>
