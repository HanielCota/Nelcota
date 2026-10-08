<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import RefreshCw from '@lucide/svelte/icons/refresh-cw'
  import Callout from './Callout.svelte'
  import { t } from '$lib/i18n/index.svelte'

  let { message, onretry, busy = false }: { message: string; onretry: () => void | Promise<void>; busy?: boolean } = $props()
</script>

<Callout variant="danger" title={t('common.loadFailed')} class="mb-4">
  <p>{message}</p>
  {#snippet actions()}
    <Button variant="outline" size="sm" disabled={busy} onclick={onretry}>
      <RefreshCw data-icon="inline-start" class={busy ? 'animate-spin' : ''} aria-hidden="true" />{t('common.retry')}
    </Button>
  {/snippet}
</Callout>
