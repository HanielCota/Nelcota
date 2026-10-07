<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { Label } from '$lib/components/ui/label'
  import { toast } from 'svelte-sonner'
  import PasswordField from './PasswordField.svelte'
  import { api, enc } from '$lib/api'
  import { passwordProblem } from '$lib/password'
  import type { User } from '$lib/types'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

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
      await api.put(`/users/${enc(user.id)}/password`, { password })
      toast.success(t('users.setPassword.saved', { email: user.email }))
      open = false
      onsaved()
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      saving = false
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-lg">
    <form class="grid gap-5" onsubmit={submit}>
      <Dialog.Header>
        <Dialog.Title>{t('users.setPassword.title')}</Dialog.Title>
        <Dialog.Description>
          {t('users.setPassword.descriptionBefore')} <span class="font-medium text-foreground">{user.email}</span>.
          {t('users.setPassword.descriptionAfter')}
        </Dialog.Description>
      </Dialog.Header>
      <div class="grid gap-2">
        <Label for="reset-password">{t('users.setPassword.newPassword')}</Label>
        <PasswordField id="reset-password" bind:value={password} />
      </div>
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (open = false)}>{t('common.cancel')}</Button>
        <Button type="submit" disabled={saving || !!passwordProblem(password)}>
          {saving ? t('common.saving') : t('users.setPassword.submit')}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
