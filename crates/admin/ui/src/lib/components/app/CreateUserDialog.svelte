<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import UserPlus from '@lucide/svelte/icons/user-plus'
  import { toast } from 'svelte-sonner'
  import PasswordField from './PasswordField.svelte'
  import { api } from '$lib/api'
  import { passwordProblem } from '$lib/password'

  let { open = $bindable(false), oncreated }: { open?: boolean; oncreated: () => void } = $props()

  let email = $state('')
  let password = $state('')
  let saving = $state(false)

  $effect(() => {
    if (open) {
      email = ''
      password = ''
    }
  })

  const ready = $derived(email.includes('@') && !passwordProblem(password))

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    saving = true
    try {
      const result = await api.post<{ message: string; email: string }>('/users', { email, password })
      toast.success(`Usuário ${result.email} criado`)
      open = false
      oncreated()
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
          <UserPlus class="size-5" />
        </span>
        <Dialog.Title>Novo usuário</Dialog.Title>
        <Dialog.Description>
          A conta já pode entrar pela API (<code>/auth/v1/token</code>) com o email e a senha abaixo.
        </Dialog.Description>
      </Dialog.Header>
      <div class="grid gap-2">
        <Label for="new-user-email">Email</Label>
        <Input id="new-user-email" type="email" bind:value={email} placeholder="ana@exemplo.com" autocomplete="off" class="h-10" required />
      </div>
      <div class="grid gap-2">
        <Label for="new-user-password">Senha</Label>
        <PasswordField id="new-user-password" bind:value={password} />
      </div>
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (open = false)}>Cancelar</Button>
        <Button type="submit" disabled={saving || !ready}>{saving ? 'Criando…' : 'Criar usuário'}</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
