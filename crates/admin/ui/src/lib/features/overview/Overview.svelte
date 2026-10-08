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
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import Table2 from '@lucide/svelte/icons/table-2'
  import Users from '@lucide/svelte/icons/users'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import SquareFunction from '@lucide/svelte/icons/square-function'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import Plus from '@lucide/svelte/icons/plus'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api } from '$lib/api'
  import { href } from '$lib/router.svelte'
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

  const stats = $derived(
    data
      ? [
          { label: 'overview.stats.tables' as MessageKey, value: data.counts.tables, path: '/tables', icon: Table2 },
          { label: 'overview.stats.users' as MessageKey, value: data.counts.users, path: '/users', icon: Users },
          { label: 'overview.stats.policies' as MessageKey, value: data.counts.policies, path: '/policies', icon: ShieldCheck },
          { label: 'overview.stats.functions' as MessageKey, value: data.counts.functions, path: '/sql', icon: SquareFunction },
        ]
      : [],
  )
</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader
    title={t('overview.title')}
    description={data ? t('overview.description', { schema: data.schema }) : undefined}
  />

  {#if failure}<LoadError message={errorMessage(failure)} onretry={load} busy={loading} />{/if}
  {#if !data && loading}
    <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
      {#each [0, 1, 2, 3] as i (i)}<Skeleton class="h-24 rounded-lg" />{/each}
    </div>
    <Skeleton class="mt-10 h-64 rounded-lg" />
  {:else if data}
    <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
      {#each stats as stat (stat.label)}
        <a
          href={href(stat.path)}
          class="group rounded-lg border bg-card px-4 py-4 transition-colors hover:border-border-strong hover:bg-muted/40 sm:px-5"
        >
          <div class="flex items-center justify-between gap-2 text-muted-foreground"><stat.icon class="size-5" aria-hidden="true" /><ArrowUpRight class="size-4 transition-colors group-hover:text-brand" aria-hidden="true" /></div>
          <p class="mt-3 text-sm text-muted-foreground">{t(stat.label)}</p>
          <p class="mt-1 text-2xl font-semibold tabular-nums">{fmt.format(stat.value)}</p>
          {#if stat.path === '/sql'}<p class="mt-1 text-xs text-muted-foreground">{t('overview.stats.inSql')}</p>{/if}
        </a>
      {/each}
    </div>

    {#if data.exposed_without_rls.length}
      <Callout
        variant="danger"
        title={t('overview.exposed.title', { tables: data.exposed_without_rls.join(', ') })}
        class="mt-6"
      >
        {t('overview.exposed.before')}
        <code class="text-xs text-foreground">alter table … enable row level security</code>
        {t('overview.exposed.after')}
        {#snippet actions()}<Button variant="outline" size="sm" href={href('/policies')}><ShieldCheck data-icon="inline-start" aria-hidden="true" />{t('common.reviewAccess')}</Button>{/snippet}
      </Callout>
    {/if}

    <section class="mt-10">
      <div class="mb-3 flex items-center justify-between gap-3">
        <h2 class="text-base font-semibold">{t('overview.tables.heading')}</h2>
        <Button variant="ghost" size="sm" href={href('/tables')}>{t('overview.tables.openEditor')}<ArrowUpRight data-icon="inline-end" aria-hidden="true" /></Button>
      </div>

      <div class="overflow-hidden rounded-lg border bg-card">
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
                <Table.Head>RLS</Table.Head>
                <Table.Head>anon</Table.Head>
                <Table.Head>authenticated</Table.Head>
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
