<script lang="ts">
  import * as AlertDialog from '$lib/components/ui/alert-dialog'
  import { buttonVariants } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import { toast } from 'svelte-sonner'
  import { ddl } from '$lib/ddl'

  let { open = $bindable(false), table, ondropped }: { open?: boolean; table: string; ondropped: () => void } = $props()

  let typed = $state('')
  let cascade = $state(false)
  let busy = $state(false)

  $effect(() => {
    if (open) {
      typed = ''
      cascade = false
    }
  })

  async function confirm() {
    busy = true
    try {
      const result = await ddl.dropTable(table, cascade)
      toast.success(result.message ?? 'Tabela apagada')
      open = false
      ondropped()
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      busy = false
    }
  }
</script>

<!-- Confirmação por digitação do nome: apagar tabela não tem volta. -->
<AlertDialog.Root bind:open>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Apagar a tabela {table}?</AlertDialog.Title>
      <AlertDialog.Description>
        Todas as linhas, policies e GRANTs da tabela somem. Não dá para desfazer (só restaurando um backup).
      </AlertDialog.Description>
    </AlertDialog.Header>
    <div class="grid gap-4">
      <label class="grid gap-2 text-sm">
        <span class="text-muted-foreground">Digite <code class="text-foreground">{table}</code> para confirmar</span>
        <Input bind:value={typed} autocomplete="off" class="font-mono" />
      </label>
      <label class="flex cursor-pointer items-start gap-3 text-sm">
        <Checkbox bind:checked={cascade} class="mt-0.5" />
        <span>
          Apagar também o que depende dela (CASCADE)
          <span class="mt-0.5 block text-xs text-muted-foreground">
            Chaves estrangeiras de outras tabelas e views que usam esta tabela.
          </span>
        </span>
      </label>
    </div>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancelar</AlertDialog.Cancel>
      <AlertDialog.Action
        class={buttonVariants({ variant: 'destructive' })}
        disabled={busy || typed !== table}
        onclick={confirm}>Apagar tabela</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
