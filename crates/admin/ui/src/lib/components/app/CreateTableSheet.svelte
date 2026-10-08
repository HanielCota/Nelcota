<script lang="ts">
  import { untrack } from 'svelte'
  import { CloseGuard } from '$lib/close-guard.svelte'
  import UnsavedChangesDialog from './UnsavedChangesDialog.svelte'
  import Save from '@lucide/svelte/icons/save'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import * as Sheet from '$lib/components/ui/sheet'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import Plus from '@lucide/svelte/icons/plus'
  import ShieldAlert from '@lucide/svelte/icons/shield-alert'
  import { toast } from 'svelte-sonner'
  import ColumnFields from './ColumnFields.svelte'
  import GrantsEditor from './GrantsEditor.svelte'
  import SqlPreview from './SqlPreview.svelte'
  import { blankColumn, ddl, type CreateTable } from '$lib/ddl'
  import { loadSchemaColumns, loadTypes } from '$lib/pg-types.svelte'
  import { SqlPreview as Preview } from '$lib/preview.svelte'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  let { open = $bindable(false), oncreated }: { open?: boolean; oncreated: (name: string) => void } = $props()

  // Supabase-style defaults: identity id, created_at, RLS on and full access
  // for service_role (everything else is granted later, with policies).
  const initial = (): CreateTable => ({
    name: '',
    comment: null,
    rls: true,
    columns: [
      { ...blankColumn(), name: 'id', data_type: 'bigint', primary_key: true, identity: true, nullable: false },
      { ...blankColumn(), name: 'created_at', data_type: 'timestamptz', default: 'now()', nullable: false },
    ],
    grants: [
      { role: 'anon', privileges: [] },
      { role: 'authenticated', privileges: [] },
      { role: 'service_role', privileges: ['select', 'insert', 'update', 'delete'] },
    ],
  })

  let spec = $state<CreateTable>(initial())
  let keys = $state<number[]>([0, 1])
  let nextKey = 2
  let tables = $state<Record<string, string[]>>({})
  let saving = $state(false)
  let initialValue = $state('')
  const guard = new CloseGuard(() => JSON.stringify(spec) !== initialValue, () => saving, () => (open = false))
  const preview = new Preview()

  $effect(() => {
    if (!open) return
    untrack(() => {
      spec = initial()
      initialValue = JSON.stringify(spec)
      keys = [0, 1]
      loadTypes()
      loadSchemaColumns()
        .then((t) => (tables = t))
        .catch(() => (tables = {}))
      })
  })

  const ready = $derived(spec.name.trim() !== '' && spec.columns.length > 0 && spec.columns.every((c) => c.name.trim()))

  // Preview on every form change (the snapshot also registers the dependency).
  $effect(() => {
    const snapshot = $state.snapshot(spec)
    if (!open || !ready) return preview.clear()
    preview.schedule((signal) => ddl.createTable(snapshot, true, { signal }))
  })

  function addColumn() {
    spec.columns.push(blankColumn())
    keys.push(nextKey++)
  }

  function removeColumn(index: number) {
    spec.columns.splice(index, 1)
    keys.splice(index, 1)
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (saving || !ready) return
    saving = true
    try {
      const result = await ddl.createTable($state.snapshot(spec))
      toast.success(t('tables.toast.tableCreated'))
      open = false
      oncreated(spec.name.trim())
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      saving = false
    }
  }
</script>

<Sheet.Root bind:open={() => open, guard.change}>
  <Sheet.Content class="flex w-full flex-col gap-0 p-0 data-[side=right]:sm:max-w-3xl">
    <Sheet.Header class="border-b px-6 pt-6 pb-5">
      <Sheet.Title>{t('tables.create.title')}</Sheet.Title>
      <Sheet.Description>{t('tables.create.description')}</Sheet.Description>
    </Sheet.Header>

    <form id="create-table" class="flex min-h-0 flex-1 flex-col gap-8 overflow-y-auto px-6 py-6" onsubmit={submit}>
      <fieldset class="contents" disabled={saving} aria-busy={saving}>
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="grid gap-2">
            <Label for="table-name">{t('tables.create.name')}</Label>
            <Input id="table-name" bind:value={spec.name} placeholder={t('tables.create.namePlaceholder')} class="font-mono" required />
          </div>
          <div class="grid gap-2">
            <Label for="table-comment">{t('tables.create.comment')}</Label>
            <Input
              id="table-comment"
              bind:value={() => spec.comment ?? '', (v) => (spec.comment = v || null)}
              placeholder={t('tables.create.optional')}
            />
          </div>
        </div>

        <label class="flex cursor-pointer items-start gap-3">
          <Checkbox bind:checked={spec.rls} class="mt-0.5" />
          <span class="text-sm font-medium">
            {t('tables.create.rls')}
            <span class="mt-0.5 block text-sm font-normal text-muted-foreground">
              {t('tables.create.rlsHintBefore')} <code>service_role</code> {t('tables.create.rlsHintAfter')}
            </span>
          </span>
        </label>
        {#if !spec.rls}
          <p class="flex gap-2 text-sm text-destructive">
            <ShieldAlert class="mt-0.5 size-4 shrink-0" />
            {t('tables.create.noRlsWarning')}
          </p>
        {/if}

        <section class="grid gap-3">
          <h3 class="text-sm font-semibold">{t('tables.create.columns')}</h3>
          {#each spec.columns as _, i (keys[i])}
            <ColumnFields
              bind:column={spec.columns[i]}
              {tables}
              mode="create"
              onremove={spec.columns.length > 1 ? () => removeColumn(i) : undefined}
            />
          {/each}
          <Button variant="outline" size="sm" class="justify-self-start" onclick={addColumn}><Plus />{t('tables.create.addColumn')}</Button>
        </section>

        <section class="grid gap-3">
          <h3 class="text-sm font-semibold">{t('tables.create.grants')}</h3>
          <GrantsEditor bind:grants={spec.grants} />
        </section>

        <SqlPreview {preview} placeholder={t('tables.create.previewPlaceholder')} />
        </fieldset>
    </form>

    <Sheet.Footer class="flex-row justify-end gap-2 border-t bg-muted/40 px-6 py-4">
      <Button variant="outline" disabled={saving} onclick={guard.request}>{t('common.cancel')}</Button>
      <Button type="submit" form="create-table" disabled={saving || !ready}>
        {#if saving}<LoaderCircle data-icon="inline-start" class="animate-spin" aria-hidden="true" />{:else}<Save data-icon="inline-start" aria-hidden="true" />{/if}
        {saving ? t('common.creating') : t('tables.create.submit')}
      </Button>
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>
<UnsavedChangesDialog bind:open={guard.pending} ondiscard={guard.discard} />
