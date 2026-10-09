<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { Button } from '$lib/components/ui/button'
  import EmptyState from '$lib/components/shared/EmptyState.svelte'
  import Callout from '$lib/components/shared/Callout.svelte'
  import RlsBadge from '$lib/shared/schema/components/RlsBadge.svelte'
  import Grants from '$lib/shared/schema/components/Grants.svelte'
  import TechnicalToggle from '$lib/shared/schema/components/TechnicalToggle.svelte'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import NextSteps from '$lib/features/overview/components/NextSteps.svelte'
  import TrafficChart from '$lib/features/overview/components/TrafficChart.svelte'
  import Ring from '$lib/features/overview/components/Ring.svelte'
  import Meter from '$lib/features/overview/components/Meter.svelte'
  import MiniBars from '$lib/features/overview/components/MiniBars.svelte'
  import Table2 from '@lucide/svelte/icons/table-2'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import Plus from '@lucide/svelte/icons/plus'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api } from '$lib/api'
  import { href } from '$lib/router.svelte'
  import { technical } from '$lib/shared/schema/technical.svelte'
  import { attention, nextSteps } from '$lib/features/overview/next-steps'
  import { groupSums, percent } from '$lib/features/overview/chart'
  import type { MetricsResponse, Overview } from '$lib/types'
  import { errorMessage, intlLocale, t } from '$lib/i18n/index.svelte'

  // The project at a glance (D98): the day's API traffic as the headline,
  // the shares that matter as rings and bars, then what needs doing.
  const resource = new RemoteResource<Overview>()
  const data = $derived(resource.data)
  const failure = $derived(resource.error)
  const loading = $derived(resource.loading)

  // The headline numbers always cover 24 h; the chart's period is a filter.
  const day = new RemoteResource<MetricsResponse>()
  const chart = new RemoteResource<MetricsResponse>()
  let range = $state<'1h' | '24h'>('24h')
  let tableView = $state(false)

  async function load() {
    await resource.load((signal) => api.get<Overview>('/overview', { signal }))
  }
  async function loadMetrics() {
    await Promise.all([
      day.load((signal) => api.get<MetricsResponse>('/metrics?range=24h', { signal })),
      chart.load((signal) => api.get<MetricsResponse>(`/metrics?range=${range}`, { signal })),
    ])
  }

  onMount(() => {
    void load()
    void loadMetrics()
    // Traffic moves on its own: refresh it every minute while the page is open.
    const timer = setInterval(() => {
      if (document.visibilityState === 'visible') void loadMetrics()
    }, 60_000)
    return () => {
      clearInterval(timer)
      resource.cancel()
      day.cancel()
      chart.cancel()
    }
  })

  function choose(next: '1h' | '24h') {
    if (range === next) return
    range = next
    void chart.load((signal) => api.get<MetricsResponse>(`/metrics?range=${range}`, { signal }))
  }

  const fmt = $derived(new Intl.NumberFormat(intlLocale()))
  const format = (n: number) => fmt.format(n)
  const time = $derived(new Intl.DateTimeFormat(intlLocale(), { hour: '2-digit', minute: '2-digit' }))
  const sinceText = $derived(
    day.data ? new Intl.DateTimeFormat(intlLocale(), { dateStyle: 'short', timeStyle: 'short' }).format(new Date(day.data.since)) : '',
  )
  const steps = $derived(data ? nextSteps(data) : [])
  const alerts = $derived(data ? attention(data) : { exposed: [], locked: [] })
  const tables = $derived(data ? data.tables.filter((table) => table.kind === 'table') : [])
  const protectedShare = $derived(percent(tables.filter((table) => table.rls.state === 'ok').length, tables.length))
  const signedInShare = $derived(data ? percent(data.counts.signed_in_users, data.counts.users) : 0)
  // Refused calls every 2 hours: twelve bars from the 96 quarter-hours.
  const refusedBars = $derived(day.data ? groupSums(day.data.points.map((p) => p.refused), 8) : [])
  const refusedLabels = $derived(day.data ? day.data.points.filter((_, i) => i % 8 === 0).map((p) => time.format(new Date(p.at))) : [])
  const policiesFor = (table: string) => href(`/policies?table=${encodeURIComponent(table)}`)

  const segment = (on: boolean) => [
    'h-9 cursor-pointer rounded-full px-4 text-sm transition-colors',
    on ? 'bg-nav-active font-medium text-nav-active-foreground' : 'text-muted-foreground hover:bg-accent hover:text-foreground',
  ]
</script>

<div class="mx-auto grid max-w-7xl gap-6 px-4 pt-6 pb-10 sm:px-6 lg:px-8">
  <!-- Headline: the page's name and the day's traffic. -->
  <section class="grid gap-6 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
    <div class="grid gap-3">
      <h1 class="text-4xl font-semibold tracking-tight sm:text-5xl">{t('overview.title')}</h1>
      <div class="flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
        {#if data}<span>{t('overview.description')}</span>{/if}
        <TechnicalToggle />
      </div>
    </div>
    {#if day.data}
      <div class="flex flex-wrap items-end gap-x-10 gap-y-6">
        <div class="grid gap-1">
          <p class="text-sm text-muted-foreground">{t('overview.hero.requests')}</p>
          <p class="text-5xl font-semibold tracking-tight">{fmt.format(day.data.totals.requests)}</p>
        </div>
        <Meter label={t('overview.hero.refused')} part={day.data.totals.refused} whole={day.data.totals.requests} {format} />
        <Meter label={t('overview.hero.errors')} part={day.data.totals.errors} whole={day.data.totals.requests} {format} />
      </div>
    {:else if day.loading}
      <Skeleton class="h-20 w-96 rounded-2xl" />
    {/if}
  </section>

  {#if failure}<LoadError message={errorMessage(failure)} onretry={load} busy={loading} />{/if}

  <!-- Stat cards beside the traffic chart. -->
  <section class="grid gap-4 lg:grid-cols-[18rem_minmax(0,1fr)]">
    <div class="grid gap-4 sm:grid-cols-3 lg:grid-cols-1">
      {#if data}
        <a href={href('/tables')} class="flex items-end justify-between gap-4 rounded-3xl bg-card p-5 transition-colors hover:bg-accent/50">
          <div class="grid gap-1">
            <p class="text-sm text-muted-foreground">{t('overview.cards.tables')}</p>
            <p class="text-4xl font-semibold tracking-tight">{fmt.format(data.counts.tables)}</p>
          </div>
          <Ring value={protectedShare} label={t('overview.cards.tablesRing', { percent: protectedShare })} />
        </a>
        <a href={href('/users')} class="flex items-end justify-between gap-4 rounded-3xl bg-card p-5 transition-colors hover:bg-accent/50">
          <div class="grid gap-1">
            <p class="text-sm text-muted-foreground">{t('overview.cards.users')}</p>
            <p class="text-4xl font-semibold tracking-tight">{fmt.format(data.counts.users)}</p>
          </div>
          <Ring value={signedInShare} label={t('overview.cards.usersRing', { percent: signedInShare })} />
        </a>
      {:else}
        <Skeleton class="h-32 rounded-3xl" />
        <Skeleton class="h-32 rounded-3xl" />
      {/if}
      {#if day.data}
        <a href={href('/policies')} class="flex items-end justify-between gap-4 rounded-3xl bg-card p-5 transition-colors hover:bg-accent/50">
          <div class="grid gap-1">
            <p class="text-sm text-muted-foreground">{t('overview.cards.refused')}</p>
            <p class="text-4xl font-semibold tracking-tight">{fmt.format(day.data.totals.refused)}</p>
          </div>
          <MiniBars values={refusedBars} labels={refusedLabels} describe={t('overview.cards.refusedBars')} />
        </a>
      {:else}
        <Skeleton class="h-32 rounded-3xl" />
      {/if}
    </div>

    <div class="grid min-w-0 content-start gap-4 rounded-3xl bg-card p-5">
      <div class="flex flex-wrap items-center gap-3">
        <h2 class="text-base font-semibold">{t('overview.traffic.title')}</h2>
        <div class="flex items-center gap-1 rounded-full bg-secondary p-1" role="group" aria-label={t('overview.traffic.range')}>
          <button type="button" class={segment(range === '1h')} aria-pressed={range === '1h'} onclick={() => choose('1h')}>{t('overview.traffic.hour')}</button>
          <button type="button" class={segment(range === '24h')} aria-pressed={range === '24h'} onclick={() => choose('24h')}>{t('overview.traffic.day')}</button>
        </div>
        {#if chart.data?.totals.p95_ms != null}
          <span class="ml-auto text-sm text-muted-foreground tabular-nums" title={t('overview.traffic.p95Title', { ms: fmt.format(chart.data.totals.p95_ms) })}
            >{t('overview.traffic.p95', { ms: fmt.format(chart.data.totals.p95_ms) })}</span
          >
        {/if}
      </div>

      {#if chart.error}
        <LoadError message={errorMessage(chart.error)} onretry={loadMetrics} busy={chart.loading} />
      {:else if chart.data}
        <!-- A refetch keeps the previous frame, dimmed. -->
        <div class={['transition-opacity', chart.loading && 'opacity-60']}>
          <TrafficChart points={chart.data.points} hourly={range === '1h'} />
        </div>
        {#if chart.data.totals.requests === 0}
          <p class="text-sm text-muted-foreground">{t('overview.traffic.empty')}</p>
        {/if}
        <div class="flex flex-wrap items-center justify-between gap-3 text-xs text-muted-foreground">
          <span>{t('overview.traffic.since', { date: sinceText })}</span>
          <button type="button" class="cursor-pointer underline-offset-4 hover:text-foreground hover:underline" onclick={() => (tableView = !tableView)}>
            {tableView ? t('overview.traffic.hideTable') : t('overview.traffic.showTable')}
          </button>
        </div>
        {#if tableView}
          <div class="max-h-72 overflow-auto rounded-2xl border">
            <Table.Root>
              <Table.Header>
                <Table.Row class="hover:bg-transparent">
                  <Table.Head>{t('overview.traffic.time')}</Table.Head>
                  <Table.Head class="text-right">{t('overview.traffic.requests')}</Table.Head>
                  <Table.Head class="text-right">{t('overview.traffic.refused')}</Table.Head>
                  <Table.Head class="text-right">{t('overview.traffic.latency')}</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each [...chart.data.points].reverse() as point (point.at)}
                  <Table.Row>
                    <Table.Cell class="tabular-nums">{time.format(new Date(point.at))}</Table.Cell>
                    <Table.Cell class="text-right tabular-nums">{fmt.format(point.requests)}</Table.Cell>
                    <Table.Cell class="text-right tabular-nums">{fmt.format(point.refused)}</Table.Cell>
                    <Table.Cell class="text-right tabular-nums">{point.p95_ms === null ? '—' : fmt.format(point.p95_ms)}</Table.Cell>
                  </Table.Row>
                {/each}
              </Table.Body>
            </Table.Root>
          </div>
        {/if}
      {:else}
        <Skeleton class="h-80 rounded-2xl" />
      {/if}
    </div>
  </section>

  {#if data}
    {#if alerts.exposed.length || alerts.locked.length}
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
    {/if}

    <NextSteps {steps} />

    <section class="grid gap-3">
      <div class="flex items-center justify-between gap-3">
        <h2 class="text-lg font-semibold">{t('overview.tables.heading')}</h2>
        <Button variant="outline" size="sm" href={href('/tables')}>{t('overview.tables.openEditor')}<ArrowUpRight data-icon="inline-end" aria-hidden="true" /></Button>
      </div>

      <div class="overflow-x-auto rounded-3xl bg-card">
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
                <Table.Head class="pl-5">{t('overview.tables.table')}</Table.Head>
                <Table.Head class="text-right">{t('overview.tables.rows')}</Table.Head>
                <Table.Head>{technical.on ? 'RLS' : t('overview.tables.protection')}</Table.Head>
                <Table.Head>{technical.on ? 'anon' : t('policies.plain.who.anon')}</Table.Head>
                <Table.Head>{technical.on ? 'authenticated' : t('policies.plain.who.authenticated')}</Table.Head>
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {#each data.tables as table (table.name)}
                <Table.Row>
                  <Table.Cell class="pl-5">
                    <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="font-medium hover:text-brand">{table.name}</a>
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
  {:else if loading}
    <Skeleton class="h-64 rounded-3xl" />
  {/if}
</div>
