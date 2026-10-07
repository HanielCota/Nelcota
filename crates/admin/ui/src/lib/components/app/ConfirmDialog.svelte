<script lang="ts">
  import * as AlertDialog from '$lib/components/ui/alert-dialog'
  import { buttonVariants } from '$lib/components/ui/button'
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert'
  import CircleHelp from '@lucide/svelte/icons/circle-help'

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
      <AlertDialog.Media
        class={[
          'size-11 rounded-xl border',
          destructive ? 'border-destructive/25 bg-destructive/10 text-destructive' : 'border-brand/20 bg-brand-soft text-brand',
        ]}
      >
        {#if destructive}<TriangleAlert class="size-5" />{:else}<CircleHelp class="size-5" />{/if}
      </AlertDialog.Media>
      <AlertDialog.Title>{title}</AlertDialog.Title>
      {#if description}
        <AlertDialog.Description class="leading-relaxed">{description}</AlertDialog.Description>
      {/if}
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel disabled={busy}>Cancelar</AlertDialog.Cancel>
      <AlertDialog.Action
        class={destructive ? buttonVariants({ variant: 'destructive' }) : ''}
        disabled={busy}
        onclick={confirm}>{busy ? 'Aguarde…' : confirmLabel}</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
