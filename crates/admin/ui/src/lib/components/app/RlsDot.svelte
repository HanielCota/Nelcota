<script lang="ts">
  import { cn } from '$lib/utils'
  import type { RlsState } from '$lib/types'

  let { state }: { state: RlsState } = $props()
  const colors: Record<RlsState, string> = {
    ok: 'bg-primary',
    warn: 'bg-warning',
    danger: 'bg-destructive',
    none: 'bg-muted-foreground/50',
    view: 'bg-muted-foreground/50',
  }
</script>

<span class={cn('size-2 shrink-0 rounded-full', colors[state])}></span>
