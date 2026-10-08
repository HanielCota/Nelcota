<script lang="ts">
  import * as AlertDialog from '$lib/components/ui/alert-dialog'
  import { buttonVariants } from '$lib/components/ui/button'
  import { t } from '$lib/i18n/index.svelte'

  let {
    open = $bindable(false),
    title,
    description,
    confirmLabel,
    destructive = false,
    onconfirm,
  }: {
    open?: boolean
    title: string
    description?: string
    confirmLabel?: string
    destructive?: boolean
    onconfirm: () => void | Promise<void>
  } = $props()

  let busy = $state(false)

  async function confirm() {
    if (busy) return
    busy = true
    try {
      await onconfirm()
      open = false
    } catch {
      // The caller already showed the error; the dialog stays open to try again.
    } finally {
      busy = false
    }
  }
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>{title}</AlertDialog.Title>
      {#if description}
        <AlertDialog.Description>{description}</AlertDialog.Description>
      {/if}
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel disabled={busy}>{t('common.cancel')}</AlertDialog.Cancel>
      <AlertDialog.Action
        class={destructive ? buttonVariants({ variant: 'destructive' }) : ''}
        disabled={busy}
        onclick={confirm}>{confirmLabel ?? t('common.confirm')}</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
