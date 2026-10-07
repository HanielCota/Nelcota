<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { Button } from '$lib/components/ui/button'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import Callout from '$lib/components/app/Callout.svelte'
  import RlsBadge from '$lib/components/app/RlsBadge.svelte'
  import Grants from '$lib/components/app/Grants.svelte'
  import { api } from '$lib/api'
  import { href } from '$lib/router.svelte'
  import type { Overview } from '$lib/types'
  import { errorMessage, intlLocale, t, type MessageKey } from '$lib/i18n/index.svelte'

  let data = $state<Overview | null>(null)
  let failure = $state<unknown>(null)

  onMount(async () => {
    try {
      data = await api.get<Overview>('/overview')
    } catch (e) {
      failure = e
    }
  })

  const fmt = $derived(new Intl.NumberFormat(intlLocale()))

  const stats = $derived(
    data
      ? [
          { label: 'overview.stats.tables' as MessageKey, value: data.counts.tables, path: '/tables' },
          { label: 'overview.stats.users' as MessageKey, value: data.counts.users, path: '/users' },
          { label: 'overview.stats.policies' as MessageKey, value: data.counts.policies, path: '/policies' },
          { label: 'overview.stats.functions' as MessageKey, value: data.counts.functions, path: '/sql' },
        ]
      : [],
  )
</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader
    title={t('overview.title')}
    description={data ? t('overview.description', { schema: data.schema }) : undefined}
  />

  {#if failure}
    <p class="text-sm text-destructive">{errorMessage(failure)}</p>
  {:else if !data}
    <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
      {#each [0, 1, 2, 3] as i (i)}<Skeleton class="h-24 rounded-lg" />{/each}
    </div>
    <Skeleton class="mt-10 h-64 rounded-lg" />
  {:else}
    <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
      {#each stats as stat (stat.label)}
        <a
          href={href(stat.path)}
          class="rounded-lg border bg-card px-5 py-4 transition-colors hover:border-border-strong hover:bg-muted/40"
        >
          <p class="text-sm text-muted-foreground">{t(stat.label)}</p>
          <p class="mt-2 text-2xl font-semibold tabular-nums">{fmt.format(stat.value)}</p>
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
      </Callout>
    {/if}

    <section class="mt-10">
      <div class="mb-3 flex items-center justify-between gap-3">
        <h2 class="text-base font-semibold">{t('overview.tables.heading')}</h2>
        <Button variant="ghost" size="sm" href={href('/tables')}>{t('overview.tables.openEditor')}</Button>
      </div>

      <div class="overflow-hidden rounded-lg border bg-card">
        {#if data.tables.length === 0}
          <EmptyState title={t('overview.tables.empty')}>
            {t('overview.tables.emptyBefore')} <code class="text-xs text-foreground">nelcota migrate</code>
            {t('overview.tables.emptyMiddle')}
            <a href={href('/sql')} class="text-brand hover:underline">{t('shell.pages.sql')}</a>.
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
