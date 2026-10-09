<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import ShieldAlert from '@lucide/svelte/icons/shield-alert'
  import Globe from '@lucide/svelte/icons/globe'
  import UserRound from '@lucide/svelte/icons/user-round'
  import ChevronDown from '@lucide/svelte/icons/chevron-down'
  import Check from '@lucide/svelte/icons/check'
  import UserPickerDialog from './UserPickerDialog.svelte'
  import { runAs, setRunAs } from '$lib/features/sql/run-as.svelte'
  import { t } from '$lib/i18n/index.svelte'

  // Who the next run acts as. The owner (full access, no rules) is the
  // default and the only amber state: it is the one to be careful with (D55).
  let picking = $state(false)
  const owner = $derived(runAs.mode === 'owner')
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger>
    {#snippet child({ props })}
      <button
        {...props}
        type="button"
        class={[
          'inline-flex max-w-64 shrink-0 cursor-pointer items-center gap-1.5 rounded-md border px-2 py-1 text-xs font-medium transition-colors',
          owner ? 'border-warning/30 bg-warning/10 text-warning hover:bg-warning/15' : 'border-border-strong bg-muted text-foreground hover:bg-accent',
        ]}
        title={owner ? t('sql.editor.ownerNoteTitle') : t('sql.runAs.label')}
        aria-label={`${t('sql.runAs.label')}: ${owner ? t('sql.runAs.owner') : runAs.mode === 'anon' ? t('sql.runAs.anon') : runAs.user?.email}`}
      >
        {#if owner}
          <ShieldAlert class="size-3.5 shrink-0" aria-hidden="true" />{t('sql.editor.ownerNote')}<span class="hidden 2xl:inline">{t('sql.editor.ownerNoteMore')}</span>
        {:else if runAs.mode === 'anon'}
          <Globe class="size-3.5 shrink-0" aria-hidden="true" />{t('sql.runAs.asVisitor')}
        {:else}
          <UserRound class="size-3.5 shrink-0" aria-hidden="true" /><span class="truncate">{t('sql.runAs.asUser', { email: runAs.user?.email ?? '' })}</span>
        {/if}
        <ChevronDown class="size-3.5 shrink-0 opacity-70" aria-hidden="true" />
      </button>
    {/snippet}
  </DropdownMenu.Trigger>
  <DropdownMenu.Content align="start" class="w-80">
    <DropdownMenu.Label class="text-xs text-muted-foreground">{t('sql.runAs.label')}</DropdownMenu.Label>
    <DropdownMenu.Item class="items-start" onclick={() => setRunAs('owner')}>
      <ShieldAlert class="mt-0.5" aria-hidden="true" />
      <span class="grid flex-1 gap-0.5"><span>{t('sql.runAs.owner')}</span><span class="text-xs text-muted-foreground">{t('sql.runAs.ownerHint')}</span></span>
      {#if owner}<Check class="mt-0.5" aria-hidden="true" />{/if}
    </DropdownMenu.Item>
    <DropdownMenu.Item class="items-start" onclick={() => setRunAs('anon')}>
      <Globe class="mt-0.5" aria-hidden="true" />
      <span class="grid flex-1 gap-0.5"><span>{t('sql.runAs.anon')}</span><span class="text-xs text-muted-foreground">{t('sql.runAs.anonHint')}</span></span>
      {#if runAs.mode === 'anon'}<Check class="mt-0.5" aria-hidden="true" />{/if}
    </DropdownMenu.Item>
    <DropdownMenu.Item class="items-start" onclick={() => (picking = true)}>
      <UserRound class="mt-0.5" aria-hidden="true" />
      <span class="grid min-w-0 flex-1 gap-0.5">
        <span>{t('sql.runAs.authenticated')}</span>
        <span class="truncate text-xs text-muted-foreground">{runAs.user ? runAs.user.email : t('sql.runAs.authenticatedHint')}</span>
      </span>
      {#if runAs.mode === 'authenticated'}<Check class="mt-0.5" aria-hidden="true" />{/if}
    </DropdownMenu.Item>
  </DropdownMenu.Content>
</DropdownMenu.Root>

<UserPickerDialog bind:open={picking} onpick={(user) => setRunAs('authenticated', user)} />
