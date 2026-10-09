<script lang="ts">
  import { onMount } from 'svelte'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { Button } from '$lib/components/ui/button'
  import Plus from '@lucide/svelte/icons/plus'
  import Pencil from '@lucide/svelte/icons/pencil'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import Search from '@lucide/svelte/icons/search'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import { Input } from '$lib/components/ui/input'
  import * as Select from '$lib/components/ui/select'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/shared/PageHeader.svelte'
  import RlsBadge from '$lib/shared/schema/components/RlsBadge.svelte'
  import PolicySheet from '$lib/features/policies/components/PolicySheet.svelte'
  import ConfirmDialog from '$lib/components/shared/ConfirmDialog.svelte'
  import Callout from '$lib/components/shared/Callout.svelte'
  import EmptyState from '$lib/components/shared/EmptyState.svelte'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import CodeBlock from '$lib/components/shared/CodeBlock.svelte'
  import TechnicalToggle from '$lib/shared/schema/components/TechnicalToggle.svelte'
  import { technical } from '$lib/shared/schema/technical.svelte'
  import { describePolicy, type PolicyMeaning } from '$lib/shared/schema/plain'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api } from '$lib/api'
  import { ddl, toPolicyDef, type PolicyDef } from '$lib/shared/schema/ddl'
  import { href, route } from '$lib/router.svelte'
  import type { PoliciesData } from '$lib/types'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  const resource = new RemoteResource<PoliciesData>()
  const data = $derived(resource.data)
  const error = $derived(resource.error ? errorMessage(resource.error) : '')
  const loading = $derived(resource.loading)
  // Arriving from an overview warning (`?table=`) opens that table.
  let search = $state(route.query.get('table') ?? '')
  let rlsFilter = $state('all')
  let enabling = $state<string | null>(null)
  const visible = $derived(data?.tables.filter((table) => table.name.toLowerCase().includes(search.trim().toLowerCase()) && (rlsFilter === 'all' || rlsFilter === 'enabled' && table.rls.enabled || rlsFilter === 'disabled' && !table.rls.enabled && table.rls.state !== 'view' || rlsFilter === 'attention' && ['danger', 'warn'].includes(table.rls.state))) ?? [])

  /** A policy in words; custom rules are named as such and keep their SQL in view. */
  function sentence(meaning: PolicyMeaning): string {
    switch (meaning.kind) {
      case 'everyoneReads':
      case 'signedInReads':
        return t(`policies.plain.rule.${meaning.kind}`)
      case 'owner':
        return t(`policies.plain.rule.owner.${meaning.command}`)
      default:
        return t('policies.plain.rule.custom')
    }
  }

  async function load() {
    await resource.load(signal => api.get<PoliciesData>('/policies', { signal }))
  }
  onMount(() => { void load(); return () => resource.cancel() })


  // Sheet: target table and the policy being edited (`null` = new).
  let sheetOpen = $state(false)
  let sheetTable = $state('')
  let editing = $state<PolicyDef | null>(null)
  let toDelete = $state<{ table: string; policy: string } | null>(null)
  let deleteOpen = $state(false)

  function openSheet(table: string, policy: PolicyDef | null) {
    sheetTable = table
    editing = policy
    sheetOpen = true
  }

  async function enableRls(table: string) {
    if (enabling) return
    enabling = table
    try {
      await ddl.alterTable(table, [{ action: 'set_rls', enabled: true }])
      toast.success(t('policies.rlsEnabled', { table }))
      await load()
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      enabling = null
    }
  }

  async function dropPolicy() {
    if (!toDelete) return
    try {
      await ddl.dropPolicy(toDelete.table, toDelete.policy)
      toast.success(t('policies.deleted', { name: toDelete.policy }))
      await load()
    } catch (e) {
      toast.error(errorMessage(e))
      throw e
    }
  }

</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader title={t('policies.title')} description={t('policies.description')}>
    {#snippet actions()}<TechnicalToggle />{/snippet}
  </PageHeader>

  {#if error}<LoadError message={error} onretry={load} busy={loading} />{/if}
  {#if !data && loading}
    <div class="grid gap-4">
      {#each [0, 1, 2] as i (i)}<Skeleton class="h-36 rounded-lg" />{/each}
    </div>
  {:else if data}
    {#if data.exposed_without_rls.length}
      <Callout variant="danger" title={t('policies.exposed', { tables: data.exposed_without_rls.join(', ') })} class="mb-6">
        {t('policies.exposedHint')}
        {#snippet actions()}<Button variant="outline" size="sm" onclick={() => (rlsFilter = 'attention')}><ShieldCheck data-icon="inline-start" aria-hidden="true" />{t('common.reviewAccess')}</Button>{/snippet}
      </Callout>
    {/if}

    {#if data.tables.length === 0}
      <EmptyState icon={ShieldCheck} title={t('policies.noTables')} description={t('policies.noTablesHint')}>{#snippet actions()}<Button href={href('/tables?create=true')}><Plus data-icon="inline-start" aria-hidden="true" />{t('tables.editor.newTable')}</Button>{/snippet}</EmptyState>
    {:else}
      <div class="mb-4 grid gap-3 sm:flex sm:items-center">
        <div class="relative min-w-0 flex-1"><Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" /><Input type="search" bind:value={search} aria-label={t('policies.search')} placeholder={t('policies.search')} class="pl-9" /></div>
        <Select.Root type="single" bind:value={rlsFilter}><Select.Trigger class="w-full sm:w-48" aria-label={t('policies.filterLabel')}>{t(`policies.filters.${rlsFilter as 'all' | 'enabled' | 'disabled' | 'attention'}`)}</Select.Trigger><Select.Content>{#each ['all', 'enabled', 'disabled', 'attention'] as option (option)}<Select.Item value={option}>{t(`policies.filters.${option as 'all' | 'enabled' | 'disabled' | 'attention'}`)}</Select.Item>{/each}</Select.Content></Select.Root>
      </div>
      {#if !visible.length}<EmptyState icon={Search} title={t('common.noMatches')}>{#snippet actions()}<Button variant="outline" onclick={() => { search = ''; rlsFilter = 'all' }}>{t('common.clearFilters')}</Button>{/snippet}</EmptyState>{/if}
    {/if}
    <div class="grid gap-4">
      {#each visible as table (table.name)}
        <section class="@container overflow-hidden rounded-lg border bg-card">
          <header class="flex flex-wrap items-center gap-3 border-b bg-muted/40 px-4 py-2.5">
            <h2 class="text-sm font-semibold">
              <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="hover:underline">{table.name}</a>
            </h2>
            <RlsBadge rls={table.rls} />
            <div class="ml-auto flex items-center gap-2">
              {#if !table.rls.enabled}
                <Button variant="outline" size="sm" disabled={enabling !== null} onclick={() => enableRls(table.name)}>{#if enabling === table.name}<LoaderCircle data-icon="inline-start" class="animate-spin" aria-hidden="true" />{:else}<ShieldCheck data-icon="inline-start" aria-hidden="true" />{/if}{t('policies.enableRls')}</Button>
              {/if}
              <Button variant="outline" size="sm" onclick={() => openSheet(table.name, null)}><Plus />{t('policies.newPolicy')}</Button>
            </div>
          </header>
          {#if table.policies.length === 0}
            <p class="px-4 py-5 text-sm text-muted-foreground">
              {#if table.rls.enabled}
                {t('policies.noPoliciesRlsBefore')} <code class="text-xs text-foreground">service_role</code> {t('policies.noPoliciesRlsAfter')}
              {:else}
                {t('policies.noPolicies')}
              {/if}
            </p>
          {:else}
            <div class="divide-y">
              {#each table.policies as policy (policy.name)}
                {@const meaning = describePolicy(policy)}
                {@const showSql = technical.on || meaning.kind === 'custom'}
                <div class={['grid grid-cols-[minmax(0,1fr)_auto] gap-3 px-4 py-3.5 text-sm', showSql && '@4xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,2fr)_auto] @4xl:items-start']}>
                  <div class="min-w-0">
                    <p class="break-words font-medium">
                      {sentence(meaning)}
                      {#if !policy.permissive}<span class="ml-1 text-xs font-normal text-muted-foreground">{t('policies.restrictiveTag')}</span>{/if}
                    </p>
                    <p class="mt-0.5 break-words text-xs text-muted-foreground">
                      {policy.name}{#if meaning.kind === 'owner'}{' · '}{t('policies.plain.rule.ownerColumn', { column: meaning.column })}{/if}
                    </p>
                  </div>
                  {#if showSql}
                  <p class="col-start-1 break-words text-xs text-muted-foreground @4xl:col-start-auto">
                    <span class="mr-1.5 rounded border px-1.5 py-px font-mono text-foreground">{policy.command}</span>
                    {policy.roles.join(', ')}
                  </p>
                  <div class="col-start-1 grid min-w-0 gap-1 font-mono text-xs @4xl:col-start-auto">
                    {#if policy.using}
                      {#if policy.using.length > 160}<details><summary class="cursor-pointer text-muted-foreground">USING · {t('common.details')}</summary><CodeBlock code={policy.using} wrap /></details>{:else}<p class="break-all"><span class="text-muted-foreground">using</span> {policy.using}</p>{/if}
                    {/if}
                    {#if policy.check}
                      {#if policy.check.length > 160}<details><summary class="cursor-pointer text-muted-foreground">WITH CHECK · {t('common.details')}</summary><CodeBlock code={policy.check} wrap /></details>{:else}<p class="break-all"><span class="text-muted-foreground">with check</span> {policy.check}</p>{/if}
                    {/if}
                  </div>
                  {/if}
                  <div class={['col-start-2 row-start-1 flex items-start justify-end gap-1', showSql && 'row-span-3 @4xl:col-start-auto @4xl:row-start-auto @4xl:row-span-1']}>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t('policies.editLabel', { name: policy.name })}
                      title={t('common.edit')}
                      onclick={() => openSheet(table.name, toPolicyDef(policy))}><Pencil /></Button
                    >
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t('policies.deleteLabel', { name: policy.name })}
                      title={t('common.delete')}
                      onclick={() => {
                        toDelete = { table: table.name, policy: policy.name }
                        deleteOpen = true
                      }}><Trash2 /></Button
                    >
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </section>
      {/each}
    </div>

    {#if data.anon_functions.length}
      <section class="mt-8 rounded-lg border bg-card p-5">
        <h2 class="text-sm font-semibold">{t('policies.anonFunctions')}</h2>
        <p class="mt-2 font-mono text-xs">{data.anon_functions.join(', ')}</p>
        <p class="mt-3 text-sm text-muted-foreground">
          {t('policies.anonFunctionsHint')}
          <code class="text-xs text-foreground">revoke execute on function f() from public</code>.
        </p>
      </section>
    {/if}
  {/if}
</div>

<PolicySheet bind:open={sheetOpen} table={sheetTable} original={editing} onsaved={load} />
{#if toDelete}
  <ConfirmDialog
    bind:open={deleteOpen}
    title={t('policies.confirmDelete.title', { name: toDelete.policy })}
    description={t('policies.confirmDelete.description', { table: toDelete.table })}
    confirmLabel={t('policies.confirmDelete.confirm')}
    destructive
    onconfirm={dropPolicy}
  />
{/if}
