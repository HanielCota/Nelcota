<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import { api } from '$lib/api'
  import { downloadText } from '$lib/download'
  import type { ExportedMigration, MigrationsData } from '$lib/types'
  import { errorMessage, hasMessage, i18n, intlLocale, t, translate } from '$lib/i18n/index.svelte'

  let data = $state<MigrationsData | null>(null)
  let error = $state('')

  async function load() {
    try {
      data = await api.get<MigrationsData>('/migrations')
      error = ''
    } catch (e) {
      error = errorMessage(e)
    }
  }

  onMount(load)

  // "Generate migration" dialog. The default name follows the panel language.
  let dialogOpen = $state(false)
  let name = $state(t('migrations.dialog.defaultName'))
  let saving = $state(false)
  // Last migration generated in this visit, to remind the next step.
  let generated = $state<ExportedMigration | null>(null)

  const validName = $derived(/^[a-z0-9][a-z0-9_]{0,59}$/.test(name))

  async function generate(event: SubmitEvent) {
    event.preventDefault()
    if (!validName) return
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

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader title={t('migrations.title')} description={t('migrations.description')} />

  {#if error}
    <p class="text-sm text-destructive">
      {error} <button type="button" class="ml-1 underline underline-offset-2" onclick={load}>{t('common.retry')}</button>
    </p>
  {:else if !data}
    <Skeleton class="h-40 rounded-lg" />
    <Skeleton class="mt-8 h-64 rounded-lg" />
  {:else}
    <section class="grid gap-3">
      <div class="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 class="text-base font-semibold">{t('migrations.pendingTitle')}</h2>
          <p class="mt-0.5 text-sm text-muted-foreground">{t('migrations.pendingHint')}</p>
        </div>
        {#if data.pending.length}
          <Button onclick={() => (dialogOpen = true)}>{t('migrations.generate')}</Button>
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
                <pre class="mt-2 overflow-x-auto rounded-md bg-muted/50 px-3 py-2 font-mono text-xs leading-relaxed">{change.statements
                    .map((s) => s.trim().replace(/;$/, '') + ';')
                    .join('\n')}</pre>
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
          description={t('migrations.noMigrationsHint')}
        />
      {:else}
        <div class="overflow-hidden rounded-lg border bg-card">
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
                  <Table.Cell class={situation.warn ? 'text-warning' : 'text-muted-foreground'}>{situation.label}</Table.Cell>
                  <Table.Cell class="text-muted-foreground">{date(migration.applied_on)}</Table.Cell>
                  <Table.Cell class="text-right">
                    {#if migration.from_panel}
                      <Button
                        variant="ghost"
                        size="sm"
                        href={`/admin/api/migrations/${migration.version}/file`}
                        download>{t('common.download')}</Button
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

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content class="sm:max-w-md">
    <form class="grid gap-5" onsubmit={generate}>
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
        <Button variant="outline" onclick={() => (dialogOpen = false)}>{t('common.cancel')}</Button>
        <Button type="submit" disabled={!validName || saving}>{saving ? t('migrations.dialog.generating') : t('migrations.dialog.submit')}</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
