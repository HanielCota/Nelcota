<script lang="ts">
  import * as AlertDialog from '$lib/components/ui/alert-dialog'
  import { buttonVariants } from '$lib/components/ui/button'

  let {
    open = $bindable(false),
    title,
    description,
    confirmLabel = 'Confirmar',
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
    busy = true
    try {
      await onconfirm()
      open = false
    } catch {
      // Quem chamou já mostrou o erro; o diálogo fica aberto para tentar de novo.
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
      <AlertDialog.Cancel>Cancelar</AlertDialog.Cancel>
      <AlertDialog.Action
        class={destructive ? buttonVariants({ variant: 'destructive' }) : ''}
        disabled={busy}
        onclick={confirm}>{confirmLabel}</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
