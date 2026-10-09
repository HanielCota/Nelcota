<script lang="ts">
  import { areaPath, monotonePath, nearestIndex, niceTicks, type Point } from '$lib/features/overview/chart'
  import type { MetricsPoint } from '$lib/types'
  import { intlLocale, t } from '$lib/i18n/index.svelte'

  // API traffic over time: requests as an area, refused calls as a line, one
  // y axis (D98). A crosshair finds the nearest time on hover or with the
  // arrow keys; the tooltip lists both series there. Colours are the
  // validated --chart-1 / --chart-2 pair.
  let { points, hourly }: { points: MetricsPoint[]; hourly: boolean } = $props()

  const HEIGHT = 300
  const PAD = { top: 16, right: 16, bottom: 32, left: 48 }
  let width = $state(0)
  let active = $state<number | null>(null)

  const plotWidth = $derived(Math.max(0, width - PAD.left - PAD.right))
  const plotHeight = HEIGHT - PAD.top - PAD.bottom
  const max = $derived(Math.max(1, ...points.map((p) => Math.max(p.requests, p.refused))))
  const ticks = $derived(niceTicks(max))
  const top = $derived(ticks[ticks.length - 1] ?? 1)
  const x = (i: number) => PAD.left + (points.length > 1 ? (i / (points.length - 1)) * plotWidth : 0)
  const y = (v: number) => PAD.top + plotHeight - (v / top) * plotHeight
  const series = (pick: (p: MetricsPoint) => number): Point[] => points.map((p, i) => ({ x: x(i), y: y(pick(p)) }))
  const requests = $derived(series((p) => p.requests))
  const refused = $derived(series((p) => p.refused))

  const fmt = $derived(new Intl.NumberFormat(intlLocale()))
  const time = $derived(new Intl.DateTimeFormat(intlLocale(), { hour: '2-digit', minute: '2-digit' }))
  // Labels every 10 minutes (last hour) or every 3 hours (last day), spaced
  // further apart when the chart is too narrow for them to fit.
  const every = $derived.by(() => {
    const base = hourly ? 10 : 12
    const fit = Math.max(1, Math.floor(plotWidth / 56))
    return base * Math.max(1, Math.ceil(points.length / base / fit))
  })
  const labels = $derived(points.map((p, i) => ({ i, text: time.format(new Date(p.at)) })).filter(({ i }) => (points.length - 1 - i) % every === 0))

  const current = $derived(active === null ? null : points[active])
  const valueText = $derived.by(() => {
    const point = current ?? points[points.length - 1]
    if (!point) return ''
    return `${time.format(new Date(point.at))}: ${fmt.format(point.requests)} ${t('overview.traffic.requests')}, ${fmt.format(point.refused)} ${t('overview.traffic.refused')}`
  })
  // Beside the crosshair, on the side with room, so it never hides the point.
  const TOOLTIP = 176
  const tooltipLeft = $derived.by(() => {
    if (active === null) return 0
    const at = x(active)
    return at > width / 2 ? Math.max(0, at - TOOLTIP - 14) : Math.min(width - TOOLTIP, at + 14)
  })

  function move(event: PointerEvent) {
    const box = (event.currentTarget as SVGElement).getBoundingClientRect()
    active = nearestIndex(event.clientX - box.left, PAD.left, plotWidth, points.length)
  }

  function key(event: KeyboardEvent) {
    if (!points.length) return
    const last = points.length - 1
    if (event.key === 'ArrowLeft') active = Math.max(0, (active ?? last) - 1)
    else if (event.key === 'ArrowRight') active = Math.min(last, (active ?? last) + 1)
    else if (event.key === 'Home') active = 0
    else if (event.key === 'End') active = last
    else if (event.key === 'Escape') active = null
    else return
    event.preventDefault()
  }
</script>

<div class="relative" bind:clientWidth={width}>
  <ul class="mb-3 flex flex-wrap gap-x-5 gap-y-1 text-xs text-muted-foreground">
    <li class="flex items-center gap-2"><span class="h-3 w-3 rounded-sm bg-chart-1" aria-hidden="true"></span>{t('overview.traffic.requests')}</li>
    <li class="flex items-center gap-2"><span class="h-0.5 w-3 rounded-full bg-chart-2" aria-hidden="true"></span>{t('overview.traffic.refused')}</li>
  </ul>

  {#if width > 0}
    <!-- The crosshair is a position in time: a slider for keyboards and
         screen readers, which hear the time and both values. -->
    <svg
      {width}
      height={HEIGHT}
      role="slider"
      tabindex="0"
      aria-label={t('overview.traffic.chartLabel')}
      aria-valuemin={0}
      aria-valuemax={Math.max(0, points.length - 1)}
      aria-valuenow={active ?? points.length - 1}
      aria-valuetext={valueText}
      class="block touch-none outline-none focus-visible:ring-2 focus-visible:ring-brand/50"
      onpointermove={move}
      onpointerleave={() => (active = null)}
      onkeydown={key}
      onblur={() => (active = null)}
    >
      {#each ticks as tick (tick)}
        <line x1={PAD.left} x2={width - PAD.right} y1={y(tick)} y2={y(tick)} class="stroke-border" stroke-width="1" />
        <text x={PAD.left - 10} y={y(tick)} dy="0.32em" text-anchor="end" class="fill-muted-foreground text-3xs tabular-nums">{fmt.format(tick)}</text>
      {/each}
      {#each labels as label (label.i)}
        <text x={x(label.i)} y={HEIGHT - 8} text-anchor="middle" class="fill-muted-foreground text-3xs tabular-nums">{label.text}</text>
      {/each}

      <path d={areaPath(requests, y(0))} class="fill-chart-1" fill-opacity="0.12" />
      <path d={monotonePath(requests)} class="stroke-chart-1" fill="none" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" />
      <path d={monotonePath(refused)} class="stroke-chart-2" fill="none" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" />

      {#if active !== null && current}
        <line x1={x(active)} x2={x(active)} y1={PAD.top} y2={PAD.top + plotHeight} class="stroke-ring" stroke-width="1" />
        <circle cx={x(active)} cy={y(current.requests)} r="4.5" class="fill-chart-1 stroke-card" stroke-width="2" />
        <circle cx={x(active)} cy={y(current.refused)} r="4.5" class="fill-chart-2 stroke-card" stroke-width="2" />
      {/if}
    </svg>
  {/if}

  {#if active !== null && current}
    <div
      class="pointer-events-none absolute top-8 z-10 w-44 rounded-2xl border bg-popover p-3 text-xs shadow-raised"
      style:left={`${tooltipLeft}px`}
      role="status"
    >
      <p class="mb-2 text-muted-foreground">{time.format(new Date(current.at))}</p>
      <p class="flex items-center gap-2">
        <span class="h-0.5 w-3 rounded-full bg-chart-1" aria-hidden="true"></span>
        <span class="text-sm font-semibold text-foreground tabular-nums">{fmt.format(current.requests)}</span>
        <span class="text-muted-foreground">{t('overview.traffic.requests')}</span>
      </p>
      <p class="mt-1 flex items-center gap-2">
        <span class="h-0.5 w-3 rounded-full bg-chart-2" aria-hidden="true"></span>
        <span class="text-sm font-semibold text-foreground tabular-nums">{fmt.format(current.refused)}</span>
        <span class="text-muted-foreground">{t('overview.traffic.refused')}</span>
      </p>
      {#if current.p95_ms !== null}
        <p class="mt-2 text-muted-foreground">{t('overview.traffic.p95', { ms: fmt.format(current.p95_ms) })}</p>
      {/if}
    </div>
  {/if}
</div>
