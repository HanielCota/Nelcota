<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { Label } from '$lib/components/ui/label'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import { toast } from 'svelte-sonner'
  import PasswordField from './PasswordField.svelte'
  import { api, enc } from '$lib/api'
  import { passwordProblem } from '$lib/password'
  import type { User } from '$lib/types'

  let { open = $bindable(false), user, onsaved }: { open?: boolean; user: User; onsaved: () => void } = $props()

  let password = $state('')
  let saving = $state(false)

  $effect(() => {
    if (open) password = ''
  })

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    saving = true
    try {
      const result = await api.put<{ message: string }>(`/users/${enc(user.id)}/password`, { password })
      toast.success(result.message)
      open = false
      onsaved()
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      saving = false
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-lg">
    <form class="grid gap-5" onsubmit={submit}>
      <Dialog.Header class="gap-2">
        <span class="grid size-11 place-items-center mb-1 rounded-xl border border-brand/20 bg-brand-soft text-brand" aria-hidden="true">
          <KeyRound class="size-5" />
        </span>
        <Dialog.Title>Redefinir a senha</Dialog.Title>
        <Dialog.Description>
          de <span class="font-semibold text-foreground">{user.email}</span>. As sessões abertas dele são encerradas.
        </Dialog.Description>
      </Dialog.Header>
      <div class="grid gap-2">
        <Label for="reset-password">Nova senha</Label>
        <PasswordField id="reset-password" bind:value={password} />
      </div>
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (open = false)}>Cancelar</Button>
        <Button type="submit" disabled={saving || !!passwordProblem(password)}>
          {saving ? 'Salvando…' : 'Redefinir senha'}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
