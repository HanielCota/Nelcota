<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import GrantsEditor from './GrantsEditor.svelte'
  import ConfirmDialog from './ConfirmDialog.svelte'
  import { API_ROLES, grantChanges, type AlterAction, type GrantDef, type Structure } from '$lib/ddl'
  import { t } from '$lib/i18n/index.svelte'

  let {
    structure,
    onalter,
    ondrop,
  }: {
    structure: Structure
    /** Applies the actions; resolves after the structure reloads. */
    onalter: (actions: AlterAction[]) => Promise<void>
    ondrop: () => void
  } = $props()

  const fromStructure = (s: Structure): GrantDef[] =>
    API_ROLES.map((role) => ({ role, privileges: [...(s.grants.find((g) => g.role === role)?.privileges ?? [])] }))

  let name = $state('')
  let comment = $state('')
  let grants = $state<GrantDef[]>([])
  let disableRlsOpen = $state(false)

  // Drafts go back to the saved state whenever the structure reloads.
  $effect(() => {
    name = structure.name
    comment = structure.comment ?? ''
    grants = fromStructure(structure)
  })

  const identityChanges = $derived.by((): AlterAction[] => {
    const actions: AlterAction[] = []
    if ((comment.trim() || null) !== structure.comment) actions.push({ action: 'set_comment', comment: comment.trim() || null })
    if (name.trim() && name.trim() !== structure.name) actions.push({ action: 'rename_table', name: name.trim() })
    return actions
  })
  const pendingGrants = $derived(grantChanges(structure.grants, grants))
</script>

<div class="grid gap-6">
  <section class="rounded-lg border bg-card p-5">
    <h2 class="text-base font-semibold">{t('tables.settings.table')}</h2>
    <form
      class="mt-3 grid gap-3 sm:grid-cols-[1fr_2fr_auto] sm:items-end"
      onsubmit={(e) => {
        e.preventDefault()
        onalter(identityChanges)
      }}
    >
      <div class="grid gap-2">
        <Label for="settings-name">{t('tables.settings.name')}</Label>
        <Input id="settings-name" bind:value={name} class="font-mono" />
      </div>
      <div class="grid gap-2">
        <Label for="settings-comment">{t('tables.settings.description')}</Label>
        <Input id="settings-comment" bind:value={comment} placeholder={t('tables.settings.descriptionPlaceholder')} />
      </div>
      <Button type="submit" variant="outline" disabled={identityChanges.length === 0}>{t('tables.settings.save')}</Button>
    </form>
  </section>

  <section class="flex flex-wrap items-center gap-4 rounded-lg border bg-card p-5">
    {#if structure.rls_enabled}
      <div class="flex-1">
        <h2 class="text-base font-semibold">{t('tables.settings.rls')}</h2>
        <p class="mt-0.5 text-sm text-muted-foreground">{t('tables.settings.rlsOn')}</p>
      </div>
      <Button variant="outline" onclick={() => (disableRlsOpen = true)}>{t('tables.settings.disable')}</Button>
    {:else}
      <div class="flex-1">
        <h2 class="text-base font-semibold">{t('tables.settings.rls')}</h2>
        <p class="mt-0.5 text-sm text-destructive">{t('tables.settings.rlsOff')}</p>
      </div>
      <Button onclick={() => onalter([{ action: 'set_rls', enabled: true }])}>{t('tables.settings.enableRls')}</Button>
    {/if}
  </section>

  <section class="grid gap-3 rounded-lg border bg-card p-5">
    <div class="flex items-center justify-between gap-3">
      <div>
        <h2 class="text-base font-semibold">{t('tables.settings.grants')}</h2>
        <p class="mt-0.5 text-sm text-muted-foreground">{t('tables.settings.grantsHint')}</p>
      </div>
      <Button variant="outline" disabled={pendingGrants.length === 0} onclick={() => onalter(pendingGrants)}>
        {t('tables.settings.saveGrants')}
      </Button>
    </div>
    <GrantsEditor bind:grants />
  </section>

  <section class="flex flex-wrap items-center gap-4 rounded-lg border border-destructive/30 p-5">
    <div class="flex-1">
      <h2 class="text-base font-semibold">{t('tables.settings.dropTitle')}</h2>
      <p class="mt-0.5 text-sm text-muted-foreground">{t('tables.settings.dropHint')}</p>
    </div>
    <Button variant="destructive" onclick={ondrop}>{t('tables.settings.dropTitle')}</Button>
  </section>
</div>

<ConfirmDialog
  bind:open={disableRlsOpen}
  title={t('tables.settings.disableRlsTitle')}
  description={t('tables.settings.disableRlsDescription')}
  confirmLabel={t('tables.settings.disable')}
  destructive
  onconfirm={() => onalter([{ action: 'set_rls', enabled: false }])}
/>
