<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import FilePlus from '@lucide/svelte/icons/file-plus'
  import Download from '@lucide/svelte/icons/download'
  import CircleCheck from '@lucide/svelte/icons/circle-check'
  import CircleAlert from '@lucide/svelte/icons/circle-alert'
  import FolderOpen from '@lucide/svelte/icons/folder-open'
  import Terminal from '@lucide/svelte/icons/terminal'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import { Badge } from '$lib/components/ui/badge'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import CodeBlock from '$lib/components/shared/CodeBlock.svelte'
  import { CloseGuard } from '$lib/close-guard.svelte'
  import UnsavedChangesDialog from '$lib/components/shared/UnsavedChangesDialog.svelte'
  import * as Table from '$lib/components/ui/table'
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/shared/PageHeader.svelte'
  import EmptyState from '$lib/components/shared/EmptyState.svelte'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api } from '$lib/api'
  import { downloadText } from '$lib/download'
  import type { ExportedMigration, MigrationsData } from '$lib/types'
  import { errorMessage, hasMessage, i18n, intlLocale, t, translate } from '$lib/i18n/index.svelte'

  const resource = new RemoteResource<MigrationsData>()
  const data = $derived(resource.data)
  const error = $derived(resource.error ? errorMessage(resource.error) : '')
  const loading = $derived(resource.loading)

  async function load() {
    await resource.load(signal => api.get<MigrationsData>('/migrations', { signal }))
  }
  onMount(() => { void load(); return () => resource.cancel() })


  // "Generate migration" dialog. The default name follows the panel language.
  let dialogOpen = $state(false)
  let name = $state(t('migrations.dialog.defaultName'))
  let saving = $state(false)
  let initialName = $state('')
  const guard = new CloseGuard(() => name !== initialName, () => saving, () => (dialogOpen = false))
  $effect(() => { if (dialogOpen) untrack(() => (initialName = name)) })
  // Last migration generated in this visit, to remind the next step.
  let generated = $state<ExportedMigration | null>(null)

  const validName = $derived(/^[a-z0-9][a-z0-9_]{0,59}$/.test(name))

  async function generate(event: SubmitEvent) {
    event.preventDefault()
    if (!validName || saving) return
    saving = true
    try {
      const result = await api.post<ExportedMigration>('/migrations', { name })
      downloadText(result.filename, result.sql, 'application/sql')
      generated = result
      dialogOpen = false
      toast.success(t('migrations.generatedToast', { filename: result.filename }))
      await load()
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      saving = false
    }
  }

  const when = $derived(new Intl.DateTimeFormat(intlLocale(), { dateStyle: 'short', timeStyle: 'short' }))
  const date = (value: string | null) => {
    if (!value) return '—'
    const parsed = new Date(value)
    return Number.isNaN(parsed.getTime()) ? value : when.format(parsed)
  }

  type Row = MigrationsData['migrations'][number]
  // Status of each migration; colour only for what needs action (D55).
  function status(m: Row): { label: string; warn: boolean } {
    if (!m.applied_on) return { label: t('migrations.status.notApplied'), warn: true }
    if (m.in_folder === false) return { label: t('migrations.status.outsideFolder'), warn: true }
    return { label: t('migrations.status.applied'), warn: false }
  }

  // In the panel language when the server recorded the kind; else the stored text.
  function describe(change: MigrationsData['pending'][number]): string {
    const key = `migrations.changes.${change.kind}`
    return change.kind && change.target !== null && hasMessage(key)
      ? translate(i18n.locale, key, { name: change.target })
      : change.summary
  }
</script>

<div class="mx-auto max-w-7xl px-4 pt-6 pb-10 sm:px-6 lg:px-8">
  <PageHeader title={t('migrations.title')} description={t('migrations.description')} />

  {#if error}<LoadError message={error} onretry={load} busy={loading} />{/if}
  {#if !data && loading}
    <Skeleton class="h-40 rounded-lg" />
    <Skeleton class="mt-8 h-64 rounded-lg" />
  {:else if data}
    <ol class="mb-6 grid gap-3 sm:grid-cols-3" aria-label={t('migrations.steps.label')}>
      <li class="rounded-lg border bg-card p-4"><div class="flex items-center gap-2 text-sm font-medium">{#if generated}<CircleCheck class="size-4 text-brand" aria-hidden="true" />{:else}<FilePlus class="size-4 text-muted-foreground" aria-hidden="true" />{/if}{t('migrations.steps.generate')}</div><p class={['mt-2 text-xs text-muted-foreground', generated && 'break-all']}>{generated ? generated.filename : t('migrations.steps.generateHint')}</p></li>
      <li class="rounded-lg border bg-card p-4"><div class="flex items-center gap-2 text-sm font-medium"><FolderOpen class="size-4 text-muted-foreground" aria-hidden="true" />{t('migrations.steps.keep')}</div><p class="mt-2 text-xs text-muted-foreground">{t('migrations.steps.keepHint')}</p>{#if generated}<Badge variant="secondary" class="mt-2">{t(data.migrations.some((migration) => migration.version === generated!.version && migration.in_folder === true) ? 'migrations.steps.inFolder' : 'migrations.steps.pending')}</Badge>{/if}</li>
      <li class="rounded-lg border bg-card p-4"><div class="flex items-center gap-2 text-sm font-medium"><Terminal class="size-4 text-muted-foreground" aria-hidden="true" />{t('migrations.steps.apply')}</div><p class="mt-2 text-xs text-muted-foreground">{t('migrations.steps.applyHint')}</p></li>
    </ol>
    <section class="grid gap-3">
      <div class="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 class="text-base font-semibold">{t('migrations.pendingTitle')}</h2>
          <p class="mt-0.5 text-sm text-muted-foreground">{t('migrations.pendingHint')}</p>
        </div>
        {#if data.pending.length}
          <Button onclick={() => (dialogOpen = true)}><FilePlus data-icon="inline-start" aria-hidden="true" />{t('migrations.generate')}</Button>
        {/if}
      </div>

      {#if generated}
        <div class="rounded-lg border px-4 py-3 text-sm">
          <p>
            <span class="font-mono">{generated.filename}</span> {t('migrations.generatedBefore')}
            <span class="font-mono">migrations/</span> {t('migrations.generatedMiddle')}
            <span class="font-mono">nelcota migrate</span> {t('migrations.generatedAfter')}
          </p>
        </div>
      {/if}

      {#if data.pending.length === 0}
        <EmptyState
          class="rounded-lg border"
          icon={CircleCheck}
          title={t('migrations.nothingPending')}
          description={t('migrations.nothingPendingHint')}
        />
      {:else}
        <ol class="divide-y rounded-lg border bg-card">
          {#each data.pending as change (change.id)}
            <li class="px-4 py-3">
              <div class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
                <p class="text-sm font-medium">{describe(change)}</p>
                <p class="text-xs text-muted-foreground">{date(change.applied_at)}</p>
              </div>
              <details class="mt-1 text-sm">
                <summary class="cursor-pointer text-muted-foreground hover:text-foreground">
                  {t('migrations.statements', { count: change.statements.length })}
                </summary>
                <div class="mt-2"><CodeBlock code={change.statements
                    .map((s) => s.trim().replace(/;$/, '') + ';')
                    .join('\n')} /></div>
              </details>
            </li>
          {/each}
        </ol>
      {/if}
    </section>

    <section class="mt-10 grid gap-3">
      <h2 class="text-base font-semibold">{t('migrations.listTitle')}</h2>
      {#if data.migrations.length === 0}
        <EmptyState
          class="rounded-lg border"
          title={t('migrations.noMigrations')}
          icon={FilePlus}
          description={t('migrations.noMigrationsHint')}
        />
      {:else}
        <div class="grid gap-3 md:hidden">
          {#each data.migrations as migration (migration.version)}
            {@const situation = status(migration)}
            <article class="grid min-w-0 gap-3 rounded-lg border bg-card p-4">
              <p class="break-words font-mono text-xs"><span class="text-muted-foreground">V{migration.version}</span> · {migration.name}</p>
              <div class="flex flex-wrap items-center gap-2"><Badge variant="outline" class={situation.warn ? 'border-warning/30 text-warning' : 'text-muted-foreground'}>{#if situation.warn}<CircleAlert aria-hidden="true" />{:else}<CircleCheck aria-hidden="true" />{/if}{situation.label}</Badge>{#if migration.from_panel}<span class="text-xs text-muted-foreground">{t('migrations.fromPanel')}</span>{/if}</div>
              <p class="text-xs text-muted-foreground">{t('migrations.columns.appliedOn')}: {date(migration.applied_on)}</p>
              {#if migration.from_panel}<Button variant="outline" size="sm" class="justify-self-start" href={`/admin/api/migrations/${migration.version}/file`} download><Download data-icon="inline-start" aria-hidden="true" />{t('common.download')}</Button>{/if}
            </article>
          {/each}
        </div>
        <div class="hidden overflow-hidden rounded-lg border bg-card md:block">
          <Table.Root>
            <Table.Header>
              <Table.Row class="hover:bg-transparent">
                <Table.Head class="w-20">{t('migrations.columns.version')}</Table.Head>
                <Table.Head>{t('migrations.columns.name')}</Table.Head>
                <Table.Head>{t('migrations.columns.status')}</Table.Head>
                <Table.Head>{t('migrations.columns.appliedOn')}</Table.Head>
                <Table.Head class="w-28"></Table.Head>
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {#each data.migrations as migration (migration.version)}
                {@const situation = status(migration)}
                <Table.Row>
                  <Table.Cell class="font-mono text-xs tabular-nums">V{migration.version}</Table.Cell>
                  <Table.Cell>
                    <span class="font-mono text-xs">{migration.name}</span>
                    {#if migration.from_panel}<span class="ml-2 text-xs text-muted-foreground">{t('migrations.fromPanel')}</span>{/if}
                  </Table.Cell>
                  <Table.Cell><Badge variant="outline" class={situation.warn ? 'border-warning/30 text-warning' : 'text-muted-foreground'}>{#if situation.warn}<CircleAlert aria-hidden="true" />{:else}<CircleCheck aria-hidden="true" />{/if}{situation.label}</Badge></Table.Cell>
                  <Table.Cell class="text-muted-foreground">{date(migration.applied_on)}</Table.Cell>
                  <Table.Cell class="text-right">
                    {#if migration.from_panel}
                      <Button
                        variant="ghost"
                        size="sm"
                        href={`/admin/api/migrations/${migration.version}/file`}
                        download><Download data-icon="inline-start" aria-hidden="true" />{t('common.download')}</Button
                      >
                    {/if}
                  </Table.Cell>
                </Table.Row>
              {/each}
            </Table.Body>
          </Table.Root>
        </div>
      {/if}
      <p class="text-sm text-muted-foreground">
        {#if data.folder}
          {t('migrations.folderRead')} <span class="font-mono">{data.folder}</span>.
        {:else}
          {t('migrations.folderMissing')}
        {/if}
      </p>
    </section>
  {/if}
</div>

<Dialog.Root bind:open={() => dialogOpen, guard.change}>
  <Dialog.Content class="sm:max-w-md">
    <form class="grid gap-5" onsubmit={generate}>
      <fieldset class="contents" disabled={saving} aria-busy={saving}>
      <Dialog.Header>
        <Dialog.Title>{t('migrations.generate')}</Dialog.Title>
        <Dialog.Description>
          {t('migrations.dialog.description', { count: data?.pending.length ?? 0 })}
        </Dialog.Description>
      </Dialog.Header>
      <div class="grid gap-2">
        <Label for="migration-name">{t('common.name')}</Label>
        <Input id="migration-name" bind:value={name} maxlength={60} autocomplete="off" aria-invalid={!validName} />
        <p class="text-sm text-muted-foreground">
          {#if validName}
            {t('migrations.dialog.file')} <span class="font-mono">V{data?.next_version}__{name}.sql</span>
          {:else}
            {t('migrations.dialog.nameRule')}
          {/if}
        </p>
      </div>
      <Dialog.Footer>
        <Button variant="outline" disabled={saving} onclick={guard.request}>{t('common.cancel')}</Button>
        <Button type="submit" disabled={!validName || saving}>{#if saving}<LoaderCircle data-icon="inline-start" class="animate-spin" aria-hidden="true" />{:else}<FilePlus data-icon="inline-start" aria-hidden="true" />{/if}{saving ? t('migrations.dialog.generating') : t('migrations.dialog.submit')}</Button>
      </Dialog.Footer>
    </fieldset>
    </form>
  </Dialog.Content>
</Dialog.Root>
<UnsavedChangesDialog bind:open={guard.pending} ondiscard={guard.discard} />
