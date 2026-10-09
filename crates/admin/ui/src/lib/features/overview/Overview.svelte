<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { Button } from '$lib/components/ui/button'
  import PageHeader from '$lib/components/shared/PageHeader.svelte'
  import EmptyState from '$lib/components/shared/EmptyState.svelte'
  import Callout from '$lib/components/shared/Callout.svelte'
  import RlsBadge from '$lib/shared/schema/components/RlsBadge.svelte'
  import Grants from '$lib/shared/schema/components/Grants.svelte'
  import TechnicalToggle from '$lib/shared/schema/components/TechnicalToggle.svelte'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import NextSteps from '$lib/features/overview/components/NextSteps.svelte'
  import Table2 from '@lucide/svelte/icons/table-2'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import Plus from '@lucide/svelte/icons/plus'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api } from '$lib/api'
  import { href } from '$lib/router.svelte'
  import { technical } from '$lib/shared/schema/technical.svelte'
  import { attention, nextSteps } from '$lib/features/overview/next-steps'
  import type { Overview } from '$lib/types'
  import { errorMessage, intlLocale, t, type MessageKey } from '$lib/i18n/index.svelte'

  const resource = new RemoteResource<Overview>()
  const data = $derived(resource.data)
  const failure = $derived(resource.error)
  const loading = $derived(resource.loading)

  async function load() {
    await resource.load(signal => api.get<Overview>('/overview', { signal }))
  }
  onMount(() => { void load(); return () => resource.cancel() })

  const fmt = $derived(new Intl.NumberFormat(intlLocale()))
  const steps = $derived(data ? nextSteps(data) : [])
  const alerts = $derived(data ? attention(data) : { exposed: [], locked: [] })
  const summary = $derived(
    data
      ? [
          { key: 'overview.summary.tables' as MessageKey, count: data.counts.tables, path: '/tables' },
          { key: 'overview.summary.users' as MessageKey, count: data.counts.users, path: '/users' },
          { key: 'overview.summary.policies' as MessageKey, count: data.counts.policies, path: '/policies' },
          { key: 'overview.summary.functions' as MessageKey, count: data.counts.functions, path: '/sql' },
        ]
      : [],
  )
  const policiesFor = (table: string) => href(`/policies?table=${encodeURIComponent(table)}`)
</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader title={t('overview.title')} description={data ? t('overview.description') : undefined}>
    {#snippet actions()}<TechnicalToggle />{/snippet}
  </PageHeader>

  {#if failure}<LoadError message={errorMessage(failure)} onretry={load} busy={loading} />{/if}
  {#if !data && loading}
    <Skeleton class="h-56 rounded-lg" />
    <Skeleton class="mt-10 h-64 rounded-lg" />
  {:else if data}
    <div class="grid gap-3">
      {#each alerts.exposed as table (table)}
        <Callout variant="danger" title={t('overview.attention.exposed', { table })}>
          {t('overview.attention.exposedHint')}
          {#snippet actions()}<Button variant="outline" size="sm" href={policiesFor(table)}>{t('overview.attention.protect')}</Button>{/snippet}
        </Callout>
      {/each}
      {#each alerts.locked as table (table)}
        <Callout title={t('overview.attention.blocked', { table })}>
          {t('overview.attention.blockedHint')}
          {#snippet actions()}<Button variant="outline" size="sm" href={policiesFor(table)}>{t('overview.attention.addRule')}</Button>{/snippet}
        </Callout>
      {/each}
    </div>

    <div class={[(alerts.exposed.length || alerts.locked.length) && 'mt-6']}>
      <NextSteps {steps} />
    </div>

    <p class="mt-6 flex flex-wrap gap-x-2 gap-y-1 text-sm text-muted-foreground">
      {#each summary as item, i (item.key)}
        {#if i > 0}<span aria-hidden="true">·</span>{/if}
        <a href={href(item.path)} class="hover:text-foreground hover:underline">{t(item.key, { count: item.count })}</a>
      {/each}
    </p>

    <section class="mt-10">
      <div class="mb-3 flex items-center justify-between gap-3">
        <h2 class="text-base font-semibold">{t('overview.tables.heading')}</h2>
        <Button variant="ghost" size="sm" href={href('/tables')}>{t('overview.tables.openEditor')}<ArrowUpRight data-icon="inline-end" aria-hidden="true" /></Button>
      </div>

      <div class="overflow-x-auto rounded-lg border bg-card">
        {#if data.tables.length === 0}
          <EmptyState icon={Table2} title={t('overview.tables.empty')}>
            {t('overview.tables.emptyBefore')} <code class="text-xs text-foreground">nelcota migrate</code>
            {t('overview.tables.emptyMiddle')}
            <a href={href('/sql')} class="text-brand hover:underline">{t('shell.pages.sql')}</a>.
            {#snippet actions()}<Button href={href('/tables?create=true')}><Plus data-icon="inline-start" aria-hidden="true" />{t('tables.editor.newTable')}</Button>{/snippet}
          </EmptyState>
        {:else}
          <Table.Root>
            <Table.Header>
              <Table.Row class="hover:bg-transparent">
                <Table.Head>{t('overview.tables.table')}</Table.Head>
                <Table.Head class="text-right">{t('overview.tables.rows')}</Table.Head>
                <Table.Head>{technical.on ? 'RLS' : t('overview.tables.protection')}</Table.Head>
                <Table.Head>{technical.on ? 'anon' : t('policies.plain.who.anon')}</Table.Head>
                <Table.Head>{technical.on ? 'authenticated' : t('policies.plain.who.authenticated')}</Table.Head>
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {#each data.tables as table (table.name)}
                <Table.Row>
                  <Table.Cell>
                    <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="font-medium hover:text-brand"
                      >{table.name}</a
                    >
                    {#if table.kind !== 'table'}<span class="ml-1.5 text-xs text-muted-foreground">{t('overview.tables.view')}</span>{/if}
                  </Table.Cell>
                  <Table.Cell class="text-right font-mono text-xs tabular-nums">
                    {table.rows === null ? '—' : `${table.rows_exact ? '' : '~'}${fmt.format(table.rows)}`}
                  </Table.Cell>
                  <Table.Cell><RlsBadge rls={table.rls} /></Table.Cell>
                  <Table.Cell><Grants grants={table.grants.anon} /></Table.Cell>
                  <Table.Cell><Grants grants={table.grants.authenticated} /></Table.Cell>
                </Table.Row>
              {/each}
            </Table.Body>
          </Table.Root>
        {/if}
      </div>
    </section>
  {/if}
</div>
