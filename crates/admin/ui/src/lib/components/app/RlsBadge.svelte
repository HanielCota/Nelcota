<script lang="ts">
  import type { Rls } from '$lib/types'
  import { t } from '$lib/i18n/index.svelte'
  import { Badge } from '$lib/components/ui/badge'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import ShieldAlert from '@lucide/svelte/icons/shield-alert'
  import ShieldOff from '@lucide/svelte/icons/shield-off'
  import Table2 from '@lucide/svelte/icons/table-2'

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

<Badge variant="outline"
  class={[
    'gap-1 whitespace-nowrap',
    rls.state === 'danger' && 'border-destructive/30 bg-destructive/10 text-destructive',
    rls.state === 'warn' && 'border-warning/30 bg-warning/10 text-warning',
    rls.state === 'ok' && 'border-brand/25 bg-brand/5 text-brand',
    (rls.state === 'none' || rls.state === 'view') && 'border-border-strong bg-muted text-muted-foreground',
  ]}
>
  {#if rls.state === 'ok'}<ShieldCheck aria-hidden="true" />{:else if rls.state === 'warn' || rls.state === 'danger'}<ShieldAlert aria-hidden="true" />{:else if rls.state === 'view'}<Table2 aria-hidden="true" />{:else}<ShieldOff aria-hidden="true" />{/if}
  {label}
</Badge>
