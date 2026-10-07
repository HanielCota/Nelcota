<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'

  let {
    open = $bindable(false),
    title,
    initialName = '',
    confirmLabel = 'Salvar',
    onsubmit,
  }: {
    open?: boolean
    title: string
    initialName?: string
    confirmLabel?: string
    onsubmit: (name: string) => void
  } = $props()

  let name = $state('')

  // Preenche o campo a cada abertura (salvar e renomear reaproveitam o diálogo).
  $effect(() => {
    if (open) name = initialName
  })

  function submit(event: SubmitEvent) {
    event.preventDefault()
    const trimmed = name.trim()
    if (!trimmed) return
    onsubmit(trimmed)
    open = false
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-md">
    <form class="grid gap-5" onsubmit={submit}>
      <Dialog.Header>
        <Dialog.Title>{title}</Dialog.Title>
        <Dialog.Description>Fica salva neste navegador, para este projeto.</Dialog.Description>
      </Dialog.Header>
      <div class="grid gap-2">
        <Label for="query-name" class="font-semibold">Nome</Label>
        <Input id="query-name" bind:value={name} placeholder="ex.: pedidos da semana" maxlength={120} class="h-10" />
        <p class="text-xs text-muted-foreground">Aparece na barra lateral e na busca (Ctrl K).</p>
      </div>
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (open = false)}>Cancelar</Button>
        <Button type="submit" disabled={!name.trim()}>{confirmLabel}</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
