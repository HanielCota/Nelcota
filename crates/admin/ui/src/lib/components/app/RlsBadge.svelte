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
    'text-xs whitespace-nowrap',
    rls.state === 'danger' && 'font-medium text-destructive',
    rls.state === 'warn' && 'text-warning',
    (rls.state === 'ok' || rls.state === 'none' || rls.state === 'view') && 'text-muted-foreground',
  ]}>{label}</span
>
