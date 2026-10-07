<script lang="ts">
  import type { Rls } from '$lib/types'
  import { t } from '$lib/i18n/index.svelte'

  let { rls }: { rls: Rls } = $props()

  // Colour only for problems: no RLS (red) and RLS without policies (amber).
  const label = $derived.by(() => {
    switch (rls.state) {
      case 'danger':
        return t('policies.rls.none')
      case 'warn':
        return t('policies.rls.noPolicies')
      case 'ok':
        return t('policies.rls.policies', { count: rls.policies })
      case 'none':
        return t('policies.rls.noGrants')
      default:
        return t('policies.rls.view')
    }
  })
</script>

<span
  class={[
    'inline-flex h-5 items-center rounded-md border px-1.5 text-xs font-medium whitespace-nowrap',
    rls.state === 'danger' && 'border-destructive/30 bg-destructive/10 text-destructive',
    rls.state === 'warn' && 'border-warning/30 bg-warning/10 text-warning',
    rls.state === 'ok' && 'border-brand/25 bg-brand/5 text-brand dark:bg-brand/10',
    (rls.state === 'none' || rls.state === 'view') && 'border-border-strong bg-muted text-muted-foreground',
  ]}
>
  {label}
</span>
