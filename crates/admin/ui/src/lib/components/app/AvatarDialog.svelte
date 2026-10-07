<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { toast } from 'svelte-sonner'
  import Avatar from './Avatar.svelte'
  import { api } from '$lib/api'
  import { prepareAvatar } from '$lib/avatar'
  import { profile } from '$lib/profile.svelte'

  let { open = $bindable(false) }: { open?: boolean } = $props()

  let input = $state<HTMLInputElement>()
  // Foto escolhida e ainda não salva.
  let picked = $state<{ dataUrl: string; base64: string } | null>(null)
  let reading = $state(false)
  let saving = $state(false)
  let dragging = $state(false)

  // Cada abertura começa sem escolha pendente.
  $effect(() => {
    if (open) picked = null
  })

  async function use(file: File | undefined) {
    if (!file) return
    if (!file.type.startsWith('image/')) {
      toast.error('Escolha um arquivo de imagem.')
      return
    }
    reading = true
    try {
      picked = await prepareAvatar(file)
    } catch {
      toast.error('Não foi possível ler essa imagem. Tente PNG, JPEG ou WebP.')
    } finally {
      reading = false
      if (input) input.value = ''
    }
  }

  function onDrop(event: DragEvent) {
    event.preventDefault()
    dragging = false
    use(event.dataTransfer?.files[0])
  }

  async function save() {
    if (!picked) return
    saving = true
    try {
      profile.avatar = (await api.put<{ avatar: number }>('/profile/avatar', { image: picked.base64 })).avatar
      toast.success('Foto atualizada')
      open = false
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      saving = false
    }
  }

  async function remove() {
    saving = true
    try {
      await api.delete('/profile/avatar')
      profile.avatar = null
      toast.success('Foto removida')
      open = false
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      saving = false
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-sm">
    <Dialog.Header>
      <Dialog.Title>Foto de perfil</Dialog.Title>
      <Dialog.Description>Aparece na barra lateral deste painel. O centro da imagem vira um círculo.</Dialog.Description>
    </Dialog.Header>

    <!-- Clique ou solte uma imagem sobre a área. -->
    <button
      type="button"
      onclick={() => input?.click()}
      ondragover={(e) => {
        e.preventDefault()
        dragging = true
      }}
      ondragleave={() => (dragging = false)}
      ondrop={onDrop}
      disabled={reading || saving}
      class={[
        'grid cursor-pointer justify-items-center gap-3 rounded-lg border border-dashed px-4 py-6 transition-colors hover:border-ring',
        dragging && 'border-brand bg-brand/5',
      ]}
    >
      <Avatar src={picked?.dataUrl ?? null} class="size-28 text-4xl" />
      <span class="text-sm text-muted-foreground">
        {reading ? 'Lendo a imagem…' : 'Clique para escolher ou arraste uma imagem aqui'}
      </span>
    </button>
    <input
      bind:this={input}
      type="file"
      accept="image/png,image/jpeg,image/webp,image/gif,image/avif"
      class="hidden"
      onchange={(e) => use(e.currentTarget.files?.[0])}
    />

    <Dialog.Footer class="sm:justify-between">
      {#if profile.avatar !== null && !picked}
        <Button variant="ghost" class="text-destructive hover:text-destructive" disabled={saving} onclick={remove}>
          Remover foto
        </Button>
      {:else}
        <span class="hidden sm:block"></span>
      {/if}
      <div class="flex flex-col-reverse gap-2 sm:flex-row">
        <Button variant="outline" onclick={() => (open = false)}>Cancelar</Button>
        <Button disabled={!picked || saving} onclick={save}>{saving ? 'Salvando…' : 'Salvar'}</Button>
      </div>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
