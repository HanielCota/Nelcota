<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import TechnicalToggle from '$lib/shared/schema/components/TechnicalToggle.svelte'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import TrafficChart from '$lib/features/overview/components/TrafficChart.svelte'
  import Ring from '$lib/features/overview/components/Ring.svelte'
  import Meter from '$lib/features/overview/components/Meter.svelte'
  import MiniBars from '$lib/features/overview/components/MiniBars.svelte'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import Maximize2 from '@lucide/svelte/icons/maximize-2'
  import Download from '@lucide/svelte/icons/download'
  import CircleCheck from '@lucide/svelte/icons/circle-check'
  import CircleAlert from '@lucide/svelte/icons/circle-alert'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api } from '$lib/api'
  import { href } from '$lib/router.svelte'
  import { downloadText, toCsv } from '$lib/download'
  import { attention, nextSteps } from '$lib/features/overview/next-steps'
  import { blockReason, blockTarget } from '$lib/features/policies/blocked'
  import { groupSums, percent } from '$lib/features/overview/chart'
  import type { DeniedRequests, MetricsResponse, Overview } from '$lib/types'
  import { errorMessage, intlLocale, t } from '$lib/i18n/index.svelte'

  // The project at a glance, laid out after the user's reference dashboard
  // (D98): the period's traffic as the headline, one period selector for the
  // whole page, stat cards beside the chart, three cards of what to look at.
  const overview = new RemoteResource<Overview>()
  const metrics = new RemoteResource<MetricsResponse>()
  const denied = new RemoteResource<DeniedRequests>()
  const data = $derived(overview.data)
  const traffic = $derived(metrics.data)
  let range = $state<'1h' | '24h'>('24h')
  let show = $state<'all' | 'requests' | 'refused'>('all')
  let tableView = $state(false)

  async function loadOverview() {
    await overview.load((signal) => api.get<Overview>('/overview', { signal }))
  }
  async function loadMetrics() {
    await metrics.load((signal) => api.get<MetricsResponse>(`/metrics?range=${range}`, { signal }))
  }
  async function loadDenied() {
    await denied.load((signal) => api.get<DeniedRequests>('/denied', { signal }))
  }

  onMount(() => {
    void loadOverview()
    void loadMetrics()
    void loadDenied()
    // Traffic moves on its own: refresh it every minute while the page is open.
    const timer = setInterval(() => {
      if (document.visibilityState !== 'visible') return
      void loadMetrics()
      void loadDenied()
    }, 60_000)
    return () => {
      clearInterval(timer)
      overview.cancel()
      metrics.cancel()
      denied.cancel()
    }
  })

  function choose(next: '1h' | '24h') {
    if (range === next) return
    range = next
    void loadMetrics()
  }

  const fmt = $derived(new Intl.NumberFormat(intlLocale()))
  const format = (n: number) => fmt.format(n)
  const time = $derived(new Intl.DateTimeFormat(intlLocale(), { hour: '2-digit', minute: '2-digit' }))
  const stamp = $derived(new Intl.DateTimeFormat(intlLocale(), { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' }))
  const periodText = $derived(range === '1h' ? t('overview.period.textHour') : t('overview.period.textDay'))
  const since = $derived(traffic ? stamp.format(new Date(traffic.since)) : '')

  const tables = $derived(data ? data.tables.filter((table) => table.kind === 'table') : [])
  const protectedShare = $derived(percent(tables.filter((table) => table.rls.state === 'ok').length, tables.length))
  const signedInShare = $derived(data ? percent(data.counts.signed_in_users, data.counts.users) : 0)
  // Twelve bars across the period, the current one in the accent.
  const refusedBars = $derived(traffic ? groupSums(traffic.points.map((p) => p.refused), Math.ceil(traffic.points.length / 12)) : [])
  const refusedLabels = $derived(
    traffic ? traffic.points.filter((_, i) => i % Math.ceil(traffic.points.length / 12) === 0).map((p) => time.format(new Date(p.at))) : [],
  )

  const totalRows = $derived(data ? data.tables.reduce((sum, table) => sum + (table.rows ?? 0), 0) : 0)
  const byRows = $derived(data ? [...data.tables].sort((a, b) => (b.rows ?? 0) - (a.rows ?? 0)).slice(0, 4) : [])
  const steps = $derived(data ? nextSteps(data).filter((step) => !step.done) : [])
  const alerts = $derived(data ? attention(data) : { exposed: [], locked: [] })
  const needs = $derived([
    ...alerts.exposed.map((table) => ({ key: `e-${table}`, title: t('overview.attention.exposed', { table }), text: t('overview.attention.exposedHint'), path: `/policies?table=${encodeURIComponent(table)}` })),
    ...alerts.locked.map((table) => ({ key: `l-${table}`, title: t('overview.attention.blocked', { table }), text: t('overview.attention.blockedHint'), path: `/policies?table=${encodeURIComponent(table)}` })),
    ...steps.map((step) => ({ key: `s-${step.id}`, title: t(`overview.steps.${step.id}.label`), text: t(`overview.steps.${step.id}.hint`), path: step.path })),
  ])
  const recentBlocks = $derived(denied.data ? denied.data.requests.slice(0, 5) : [])

  function download() {
    if (!traffic) return
    const rows = traffic.points.map((p) => [p.at, String(p.requests), String(p.refused), String(p.errors), p.p95_ms === null ? null : String(p.p95_ms)])
    downloadText(`nelcota-trafego-${range}.csv`, toCsv(['at', 'requests', 'refused', 'errors', 'p95_ms'], rows), 'text/csv')
  }

  const segment = (on: boolean) => [
    'h-9 cursor-pointer rounded-full px-3.5 text-sm transition-colors',
    on ? 'bg-secondary font-medium text-foreground' : 'text-muted-foreground hover:text-foreground',
  ]
  const filter = (on: boolean) => [
    'h-10 cursor-pointer rounded-full border px-4 text-sm transition-colors',
    on ? 'border-transparent bg-nav-active font-medium text-nav-active-foreground' : 'border-border-strong text-muted-foreground hover:text-foreground',
  ]
  const card = 'rounded-3xl bg-card p-5'
  const statLink = 'grid size-7 place-items-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground'
</script>

{#snippet cardHeader(title: string, path: string)}
  <div class="flex items-center justify-between gap-3">
    <h2 class="text-sm font-medium text-muted-foreground">{title}</h2>
    <a href={href(path)} class="grid size-10 place-items-center rounded-full bg-secondary text-muted-foreground transition-colors hover:text-foreground" aria-label={`${t('overview.open')}: ${title}`}>
      <Maximize2 class="size-4" aria-hidden="true" />
    </a>
  </div>
{/snippet}

<div class="grid gap-6 px-4 pt-2 pb-12 sm:px-6 lg:px-8">
  <!-- Headline: the page's name and the period's traffic. -->
  <section class="grid gap-8 xl:grid-cols-[minmax(0,1fr)_auto] xl:items-end">
    <h1 class="max-w-xl text-5xl leading-[1.02] font-semibold tracking-[-0.035em]">{t('overview.title')}</h1>
    {#if traffic}
      <div class="flex flex-wrap items-end gap-x-12 gap-y-6">
        <div class="grid gap-1">
          <p class="text-sm text-muted-foreground">{t('overview.hero.requests')}</p>
          <p class="text-5xl leading-none font-semibold tracking-[-0.035em]">{fmt.format(traffic.totals.requests)}</p>
        </div>
        <div class="w-64"><Meter label={t('overview.hero.refused')} part={traffic.totals.refused} whole={traffic.totals.requests} {format} /></div>
        <div class="w-64"><Meter label={t('overview.hero.errors')} part={traffic.totals.errors} whole={traffic.totals.requests} {format} /></div>
      </div>
    {:else if metrics.loading}
      <Skeleton class="h-16 w-[40rem] max-w-full rounded-2xl" />
    {/if}
  </section>

  <!-- One period for the whole page. -->
  <section class="flex flex-wrap items-center justify-between gap-3">
    <p class="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted-foreground">
      <span>{periodText}{#if since} · {t('overview.period.since', { date: since })}{/if}</span>
      <TechnicalToggle />
    </p>
    <div class="flex items-center gap-1 rounded-full bg-card p-1" role="group" aria-label={t('overview.period.label')}>
      <button type="button" class={segment(range === '1h')} aria-pressed={range === '1h'} onclick={() => choose('1h')}>{t('overview.period.hour')}</button>
      <button type="button" class={segment(range === '24h')} aria-pressed={range === '24h'} onclick={() => choose('24h')}>{t('overview.period.day')}</button>
    </div>
  </section>

  {#if overview.error}<LoadError message={errorMessage(overview.error)} onretry={loadOverview} busy={overview.loading} />{/if}

  <!-- Stat cards beside the traffic chart. -->
  <section class="grid gap-4 lg:grid-cols-[18.75rem_minmax(0,1fr)]">
    <div class="grid content-start gap-4 sm:grid-cols-3 lg:grid-cols-1">
      {#if data}
        <article class={card}>
          <div class="flex items-center justify-between text-xs text-muted-foreground">
            <span>{t('overview.cards.now')}</span>
            <a href={href('/tables')} class={statLink} aria-label={t('overview.cards.tables')}><ArrowUpRight class="size-4" aria-hidden="true" /></a>
          </div>
          <div class="mt-4 flex items-end justify-between gap-4">
            <div class="grid gap-1">
              <p class="text-sm text-muted-foreground">{t('overview.cards.tables')}</p>
              <p class="text-3xl leading-none font-semibold tracking-[-0.03em]">{fmt.format(data.counts.tables)}</p>
            </div>
            <Ring value={protectedShare} label={t('overview.cards.tablesRing', { percent: protectedShare })} />
          </div>
        </article>
        <article class={card}>
          <div class="flex items-center justify-between text-xs text-muted-foreground">
            <span>{t('overview.cards.now')}</span>
            <a href={href('/users')} class={statLink} aria-label={t('overview.cards.users')}><ArrowUpRight class="size-4" aria-hidden="true" /></a>
          </div>
          <div class="mt-4 flex items-end justify-between gap-4">
            <div class="grid gap-1">
              <p class="text-sm text-muted-foreground">{t('overview.cards.users')}</p>
              <p class="text-3xl leading-none font-semibold tracking-[-0.03em]">{fmt.format(data.counts.users)}</p>
            </div>
            <Ring value={signedInShare} label={t('overview.cards.usersRing', { percent: signedInShare })} />
          </div>
        </article>
      {:else}
        <Skeleton class="h-36 rounded-3xl" />
        <Skeleton class="h-36 rounded-3xl" />
      {/if}
      {#if traffic}
        <article class={card}>
          <div class="flex items-center justify-between text-xs text-muted-foreground">
            <span>{periodText}</span>
            <a href={href('/policies')} class={statLink} aria-label={t('overview.cards.refused')}><ArrowUpRight class="size-4" aria-hidden="true" /></a>
          </div>
          <div class="mt-4 flex items-end justify-between gap-4">
            <div class="grid gap-1">
              <p class="text-sm text-muted-foreground">{t('overview.cards.refused')}</p>
              <p class="text-3xl leading-none font-semibold tracking-[-0.03em]">{fmt.format(traffic.totals.refused)}</p>
            </div>
            <MiniBars values={refusedBars} labels={refusedLabels} describe={t('overview.cards.refusedBars')} />
          </div>
        </article>
      {:else}
        <Skeleton class="h-36 rounded-3xl" />
      {/if}
    </div>

    <article class={['grid min-w-0 content-start gap-4', card]}>
      <div class="flex flex-wrap items-center gap-2">
        <div class="flex flex-wrap items-center gap-2" role="group" aria-label={t('overview.filters.label')}>
          <button type="button" class={filter(show === 'all')} aria-pressed={show === 'all'} onclick={() => (show = 'all')}>{t('overview.filters.all')}</button>
          <button type="button" class={filter(show === 'requests')} aria-pressed={show === 'requests'} onclick={() => (show = 'requests')}>
            <span class="mr-1.5 inline-block size-2 rounded-full bg-chart-1 align-middle" aria-hidden="true"></span>{t('overview.filters.requests')}
          </button>
          <button type="button" class={filter(show === 'refused')} aria-pressed={show === 'refused'} onclick={() => (show = 'refused')}>
            <span class="mr-1.5 inline-block size-2 rounded-full bg-chart-2 align-middle" aria-hidden="true"></span>{t('overview.filters.refused')}
          </button>
        </div>
        <div class="ml-auto flex items-center gap-3">
          {#if traffic?.totals.p95_ms != null}
            <span class="text-sm text-muted-foreground tabular-nums" title={t('overview.traffic.p95Title', { ms: fmt.format(traffic.totals.p95_ms) })}
              >{t('overview.traffic.p95', { ms: fmt.format(traffic.totals.p95_ms) })}</span
            >
          {/if}
          <button
            type="button"
            class="grid size-10 cursor-pointer place-items-center rounded-full border border-border-strong text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
            onclick={download}
            disabled={!traffic}
            aria-label={t('overview.download')}
            title={t('overview.download')}><Download class="size-4" aria-hidden="true" /></button
          >
        </div>
      </div>

      {#if metrics.error}
        <LoadError message={errorMessage(metrics.error)} onretry={loadMetrics} busy={metrics.loading} />
      {:else if traffic}
        <!-- A refetch keeps the previous frame, dimmed. -->
        <div class={['transition-opacity', metrics.loading && 'opacity-60']}>
          <TrafficChart points={traffic.points} hourly={range === '1h'} {show} />
        </div>
        <div class="flex flex-wrap items-center justify-between gap-3 text-xs text-muted-foreground">
          <span>{traffic.totals.requests === 0 ? t('overview.traffic.empty') : t('overview.traffic.since', { date: since })}</span>
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
                {#each [...traffic.points].reverse() as point (point.at)}
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
        <Skeleton class="h-[22rem] rounded-2xl" />
      {/if}
    </article>
  </section>

  <!-- Three cards: where the data is, what was refused, what needs doing. -->
  <section class="grid gap-4 lg:grid-cols-3">
    <article class={['grid content-start gap-5', card]}>
      {@render cardHeader(t('overview.rows.title'), '/tables')}
      {#if !data}
        <Skeleton class="h-40 rounded-2xl" />
      {:else if byRows.length === 0}
        <p class="text-sm text-muted-foreground">{t('overview.rows.empty')}</p>
      {:else}
        <ul class="grid gap-5">
          {#each byRows as table (table.name)}
            {@const share = percent(table.rows ?? 0, totalRows)}
            <li class="grid gap-2">
              <div class="flex items-baseline justify-between gap-3">
                <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="truncate font-medium hover:text-brand">{table.name}</a>
                <span class="font-medium tabular-nums">{table.rows === null ? '—' : `${table.rows_exact ? '' : '~'}${fmt.format(table.rows)}`}</span>
              </div>
              <div class="h-1.5 overflow-hidden rounded-full bg-secondary"><div class="h-full rounded-full bg-brand" style:width={`${share}%`}></div></div>
              <p class="text-xs text-muted-foreground">{t('overview.rows.detail', { percent: share, protection: t(`policies.plain.state.${table.rls.state}`) })}</p>
            </li>
          {/each}
        </ul>
      {/if}
    </article>

    <article class={['grid content-start gap-4', card]}>
      {@render cardHeader(t('overview.blocked.title'), '/policies')}
      {#if !denied.data}
        <Skeleton class="h-40 rounded-2xl" />
      {:else if recentBlocks.length === 0}
        <p class="text-sm text-muted-foreground">{t('overview.blocked.none')}</p>
      {:else}
        <table class="w-full text-sm">
          <thead>
            <tr class="text-xs text-muted-foreground">
              <th class="pb-2 text-left font-normal">{t('overview.blocked.who')}</th>
              <th class="pb-2 text-left font-normal">{t('overview.blocked.where')}</th>
              <th class="pb-2 text-right font-normal">{t('overview.blocked.when')}</th>
            </tr>
          </thead>
          <tbody>
            {#each recentBlocks as block, i (`${block.at}-${i}`)}
              {@const target = blockTarget(block.path)}
              <tr class="border-t" title={t(`policies.blocked.reasons.${blockReason(block)}`)}>
                <td class="max-w-28 truncate py-2.5 pr-3">{block.email ?? t(`policies.blocked.who.${block.role === 'anon' || block.role === 'invalid_token' || block.role === 'service_role' ? block.role : 'authenticated'}`)}</td>
                <td class="max-w-32 truncate py-2.5 pr-3 text-muted-foreground">{target ? target.name : block.path}</td>
                <td class="py-2.5 text-right text-muted-foreground tabular-nums">{time.format(new Date(block.at))}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </article>

    <article class={['grid content-start gap-4', card]}>
      <h2 class="text-sm font-medium text-muted-foreground">{t('overview.needs.title')}</h2>
      {#if !data}
        <Skeleton class="h-40 rounded-2xl" />
      {:else if needs.length === 0}
        <div class="flex gap-3">
          <CircleCheck class="mt-0.5 size-5 shrink-0 text-brand" aria-hidden="true" />
          <div>
            <p class="font-medium">{t('overview.needs.allGood')}</p>
            <p class="text-sm text-muted-foreground">{t('overview.needs.allGoodText')}</p>
          </div>
        </div>
      {:else}
        <ul class="grid gap-4">
          {#each needs.slice(0, 4) as item (item.key)}
            <li>
              <a href={href(item.path)} class="group flex gap-3">
                <CircleAlert class="mt-0.5 size-5 shrink-0 text-warning" aria-hidden="true" />
                <span class="grid gap-0.5">
                  <span class="font-medium group-hover:text-brand">{item.title}</span>
                  <span class="text-sm text-muted-foreground">{item.text}</span>
                </span>
              </a>
            </li>
          {/each}
        </ul>
      {/if}
    </article>
  </section>
</div>
