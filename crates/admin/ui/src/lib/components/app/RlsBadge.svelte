<script lang="ts">
  import type { Rls } from '$lib/types'

  let { rls }: { rls: Rls } = $props()

  // Cor só para problema: sem RLS (vermelho) e RLS sem policies (âmbar).
  const label = $derived.by(() => {
    switch (rls.state) {
      case 'danger':
        return 'sem RLS'
      case 'warn':
        return 'RLS sem policies'
      case 'ok':
        return `RLS, ${rls.policies} ${rls.policies === 1 ? 'policy' : 'policies'}`
      case 'none':
        return 'sem RLS, sem grants'
      default:
        return 'view'
    }
  })
</script>

<span
  class={[
    'inline-flex h-5 items-center gap-1.5 rounded-full border px-2 text-[11px] font-medium whitespace-nowrap',
    rls.state === 'danger' && 'border-destructive/30 bg-destructive/10 text-destructive',
    rls.state === 'warn' && 'border-warning/30 bg-warning/10 text-warning',
    rls.state === 'ok' && 'border-brand/25 bg-brand/10 text-brand',
    (rls.state === 'none' || rls.state === 'view') && 'border-border-strong bg-muted text-muted-foreground',
  ]}
>
  <span class="size-1.5 rounded-full bg-current"></span>{label}
</span>
