<script lang="ts">
  import { CloseGuard } from '$lib/close-guard.svelte'
  import UnsavedChangesDialog from '$lib/components/shared/UnsavedChangesDialog.svelte'
  import UserPlus from '@lucide/svelte/icons/user-plus'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { toast } from 'svelte-sonner'
  import PasswordField from '$lib/components/shared/PasswordField.svelte'
  import { api } from '$lib/api'
  import { passwordProblem } from '$lib/password'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  let { open = $bindable(false), oncreated }: { open?: boolean; oncreated: () => void } = $props()

  let email = $state('')
  let password = $state('')
  let saving = $state(false)
  const guard = new CloseGuard(() => email !== '' || password !== '', () => saving, () => (open = false))

  $effect(() => {
    if (open) {
      email = ''
      password = ''
    }
  })

  const ready = $derived(email.includes('@') && !passwordProblem(password))

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (saving || !ready) return
    saving = true
    try {
      const result = await api.post<{ email: string }>('/users', { email, password })
      toast.success(t('users.create.created', { email: result.email }))
      open = false
      oncreated()
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      saving = false
    }
  }
</script>

<Dialog.Root bind:open={() => open, guard.change}>
  <Dialog.Content class="sm:max-w-lg">
    <form class="grid gap-5" onsubmit={submit}>
      <fieldset class="contents" disabled={saving} aria-busy={saving}>
        <Dialog.Header>
          <Dialog.Title>{t('users.new')}</Dialog.Title>
          <Dialog.Description>
            {t('users.create.descriptionBefore')} (<code>/auth/v1/token</code>) {t('users.create.descriptionAfter')}
          </Dialog.Description>
        </Dialog.Header>
        <div class="grid gap-2">
          <Label for="new-user-email">{t('users.columns.email')}</Label>
          <Input id="new-user-email" type="email" bind:value={email} placeholder={t('users.create.emailPlaceholder')} autocomplete="off" class="h-10" required />
        </div>
        <div class="grid gap-2">
          <Label for="new-user-password">{t('users.create.password')}</Label>
          <PasswordField id="new-user-password" bind:value={password} />
        </div>
        <Dialog.Footer>
          <Button variant="outline" disabled={saving} onclick={guard.request}>{t('common.cancel')}</Button>
          <Button type="submit" disabled={saving || !ready}>{#if saving}<LoaderCircle data-icon="inline-start" class="animate-spin" aria-hidden="true" />{:else}<UserPlus data-icon="inline-start" aria-hidden="true" />{/if}{saving ? t('common.creating') : t('users.create.submit')}</Button>
        </Dialog.Footer>
        </fieldset>
    </form>
  </Dialog.Content>
</Dialog.Root>
<UnsavedChangesDialog bind:open={guard.pending} ondiscard={guard.discard} />
