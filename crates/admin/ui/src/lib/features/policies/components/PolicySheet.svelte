<script lang="ts">
  import { untrack } from 'svelte'
  import { CloseGuard } from '$lib/close-guard.svelte'
  import UnsavedChangesDialog from '$lib/components/shared/UnsavedChangesDialog.svelte'
  import Save from '@lucide/svelte/icons/save'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import FileCode from '@lucide/svelte/icons/file-code'
  import * as Sheet from '$lib/components/ui/sheet'
  import * as Select from '$lib/components/ui/select'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import { Textarea } from '$lib/components/ui/textarea'
  import { toast } from 'svelte-sonner'
  import SqlPreview from '$lib/shared/schema/components/SqlPreview.svelte'
  import { ddl, policyFields, type ApiRole, type PolicyCommand, type PolicyDef } from '$lib/shared/schema/ddl'
  import { POLICY_TEMPLATES, guessOwnerColumn, templateText, type PolicyTemplate } from '$lib/features/policies/policy-templates'
  import { SqlPreview as Preview } from '$lib/shared/schema/preview.svelte'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  let {
    open = $bindable(false),
    table,
    original = null,
    onsaved,
  }: {
    open?: boolean
    table: string
    /** Policy being edited; `null` = new. */
    original?: PolicyDef | null
    onsaved: () => void
  } = $props()

  const COMMANDS: PolicyCommand[] = ['select', 'insert', 'update', 'delete', 'all']
  const ROLES: ApiRole[] = ['anon', 'authenticated', 'service_role']

  const blank = (): PolicyDef => ({
    name: '',
    command: 'select',
    roles: ['authenticated'],
    permissive: true,
    using: '',
    check: '',
  })

  let policy = $state<PolicyDef>(blank())
  let ownerColumn = $state('user_id')
  let saving = $state(false)
  let initialValue = $state('')
  const guard = new CloseGuard(() => JSON.stringify(policy) !== initialValue, () => saving, () => (open = false))
  const preview = new Preview()
  const fields = $derived(policyFields(policy.command))

  $effect(() => {
    if (!open) return
    untrack(() => {
      policy = original ? { ...original, roles: [...original.roles] } : blank()
      initialValue = JSON.stringify(policy)
      // The owner column (uuid) feeds the templates.
      ddl
        .structure(table)
        .then((s) => (ownerColumn = guessOwnerColumn(s.columns)))
        .catch(() => (ownerColumn = 'user_id'))
      })
  })

  /** Only sends the expressions the command accepts. */
  const payload = $derived<PolicyDef>({
    ...policy,
    name: policy.name.trim(),
    using: fields.using ? policy.using?.trim() || null : null,
    check: fields.check ? policy.check?.trim() || null : null,
  })
  const ready = $derived(payload.name !== '' && (payload.command === 'insert' ? !!payload.check : !!payload.using))

  $effect(() => {
    const snapshot = $state.snapshot(payload)
    if (!open || !ready) return preview.clear()
    preview.schedule((signal) =>
      original
        ? ddl.replacePolicy(table, original.name, snapshot, true, { signal })
        : ddl.createPolicy(table, snapshot, true, { signal }),
    )
  })

  function applyTemplate(template: PolicyTemplate) {
    const built = template.build(ownerColumn)
    policy = {
      ...built,
      name: templateText(template.id).name,
      permissive: true,
      using: built.using ?? '',
      check: built.check ?? '',
    }
  }

  function toggleRole(role: ApiRole, on: boolean) {
    policy.roles = on ? [...policy.roles, role] : policy.roles.filter((r) => r !== role)
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (saving || !ready) return
    saving = true
    try {
      const snapshot = $state.snapshot(payload)
      if (original) await ddl.replacePolicy(table, original.name, snapshot)
      else await ddl.createPolicy(table, snapshot)
      toast.success(t('policies.sheet.saved', { name: snapshot.name }))
      open = false
      onsaved()
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      saving = false
    }
  }

  const commandLabel = (value: PolicyCommand) => t(`policies.sheet.commands.${value}`)
</script>

<Sheet.Root bind:open={() => open, guard.change}>
  <Sheet.Content class="flex w-full flex-col gap-0 p-0 data-[side=right]:sm:max-w-2xl">
    <Sheet.Header class="border-b px-6 py-4">
      <Sheet.Title>{original ? t('policies.sheet.edit') : t('policies.sheet.new')}</Sheet.Title>
      <Sheet.Description>{t('policies.sheet.onTable')} <code class="font-mono text-xs text-foreground">{table}</code></Sheet.Description>
    </Sheet.Header>

    <form id="policy-form" class="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-6 py-6" onsubmit={submit}>
      <fieldset class="contents" disabled={saving} aria-busy={saving}>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button variant="outline" {...props}><FileCode data-icon="inline-start" aria-hidden="true" />{t('policies.sheet.fromTemplate')}</Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="start" class="w-96 max-w-[calc(100vw-2rem)]">
            <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">
              {t('policies.sheet.templatesOwnerColumn')} <code class="text-foreground">{ownerColumn}</code>
            </DropdownMenu.Label>
            {#each POLICY_TEMPLATES as template (template.id)}
              {@const text = templateText(template.id)}
              <DropdownMenu.Item onclick={() => applyTemplate(template)} class="flex-col items-start gap-0.5 py-2">
                <span class="font-medium">{text.label}</span>
                <span class="text-xs text-muted-foreground">{text.description}</span>
              </DropdownMenu.Item>
            {/each}
          </DropdownMenu.Content>
        </DropdownMenu.Root>

        <div class="grid gap-4 sm:grid-cols-2">
          <div class="grid gap-1.5">
            <Label for="policy-name">{t('policies.sheet.name')}</Label>
            <Input id="policy-name" bind:value={policy.name} placeholder={t('policies.sheet.namePlaceholder')} required />
          </div>
          <div class="grid gap-1.5">
            <Label for="policy-command">{t('policies.sheet.command')}</Label>
            <Select.Root type="single" bind:value={policy.command}>
              <Select.Trigger id="policy-command" class="w-full">{commandLabel(policy.command)}</Select.Trigger>
              <Select.Content>
                {#each COMMANDS as command (command)}
                  <Select.Item value={command}>{commandLabel(command)}</Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        </div>

        <fieldset class="grid gap-2">
          <legend class="mb-1 text-sm font-medium">{t('policies.sheet.appliesTo')}</legend>
          <div class="grid gap-2 sm:grid-cols-3">
            {#each ROLES as role (role)}
              <label
                class="flex cursor-pointer items-center gap-3 rounded-md border bg-card px-3 py-2.5 text-sm transition-colors hover:border-border-strong has-data-checked:border-brand/50"
              >
                <Checkbox checked={policy.roles.includes(role)} onCheckedChange={(v) => toggleRole(role, v === true)} />
                <span class="grid">
                  <span class="font-mono text-xs font-medium">{role}</span>
                  <span class="text-xs text-muted-foreground">{t(`policies.sheet.roles.${role}`)}</span>
                </span>
              </label>
            {/each}
          </div>
          {#if policy.roles.length === 0}
            <p class="text-xs text-muted-foreground">{t('policies.sheet.noRoles')}</p>
          {/if}
          </fieldset>

      <label class="flex cursor-pointer items-start gap-3 text-sm">
        <Checkbox checked={!policy.permissive} onCheckedChange={(v) => (policy.permissive = v !== true)} class="mt-0.5" />
        <span>
          <span class="font-medium">{t('policies.sheet.restrictive')}</span>
          <span class="mt-0.5 block text-xs text-muted-foreground">{t('policies.sheet.restrictiveHint')}</span>
        </span>
      </label>

      {#if fields.using}
        <div class="grid gap-1.5">
          <Label for="policy-using" class="flex-wrap">
            <span class="font-mono">USING</span> <span class="font-normal text-muted-foreground">{policy.command === 'delete' ? t('policies.sheet.usingDeletes') : t('policies.sheet.usingSees')}</span>
          </Label>
          <Textarea
            id="policy-using"
            bind:value={() => policy.using ?? '', (v) => (policy.using = v)}
            placeholder={`${ownerColumn} = auth.uid()`}
            class="min-h-24 font-mono text-xs"
          />
        </div>
      {/if}
      {#if fields.check}
        <div class="grid gap-1.5">
          <Label for="policy-check" class="flex-wrap">
            <span class="font-mono">WITH CHECK</span> <span class="font-normal text-muted-foreground">{t('policies.sheet.checkHint')}</span>
          </Label>
          <Textarea
            id="policy-check"
            bind:value={() => policy.check ?? '', (v) => (policy.check = v)}
            placeholder={policy.command === 'insert' ? `${ownerColumn} = auth.uid()` : t('policies.sheet.checkPlaceholder')}
            class="min-h-24 font-mono text-xs"
          />
        </div>
      {/if}
      <p class="text-xs text-muted-foreground">
        {t('policies.sheet.helpBefore')} <code class="text-foreground">auth.uid()</code> {t('policies.sheet.helpMiddle')}
        <code class="text-foreground">auth.jwt()</code> {t('policies.sheet.helpAfter')}
      </p>

      <SqlPreview {preview} placeholder={t('policies.sheet.previewPlaceholder')} />
    </fieldset>
    </form>

    <Sheet.Footer class="flex-row justify-end gap-2 border-t bg-muted/40 px-6 py-4">
      <Button variant="outline" disabled={saving} onclick={guard.request}>{t('common.cancel')}</Button>
      <Button type="submit" form="policy-form" disabled={saving || !ready}>
        {#if saving}<LoaderCircle data-icon="inline-start" class="animate-spin" aria-hidden="true" />{:else}<Save data-icon="inline-start" aria-hidden="true" />{/if}
        {saving ? t('common.saving') : original ? t('policies.sheet.savePolicy') : t('policies.sheet.createPolicy')}
      </Button>
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>
<UnsavedChangesDialog bind:open={guard.pending} ondiscard={guard.discard} />
