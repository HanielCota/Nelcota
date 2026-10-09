<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import RefreshCw from '@lucide/svelte/icons/refresh-cw'
  import Plus from '@lucide/svelte/icons/plus'
  import ChevronLeft from '@lucide/svelte/icons/chevron-left'
  import Funnel from '@lucide/svelte/icons/funnel'
  import Download from '@lucide/svelte/icons/download'
  import Columns3 from '@lucide/svelte/icons/columns-3'
  import RlsBadge from '$lib/shared/schema/components/RlsBadge.svelte'
  import RunAsPicker from '$lib/components/shared/RunAsPicker.svelte'
  import type { RunAsLabels, Viewer } from '$lib/shared/run-as'
  import { enc } from '$lib/api'
  import { href, route } from '$lib/router.svelte'
  import type { TableData } from '$lib/types'
  import { t } from '$lib/i18n/index.svelte'

  let {
    name,
    view,
    data,
    filterCount,
    filterOpen = $bindable(false),
    loading,
    hiddenColumns,
    ontogglecolumn,
    onshowallcolumns,
    exportHref,
    onreload,
    oninsert,
    viewer,
    viewerLabels,
    onviewer,
  }: {
    name: string
    view: 'data' | 'structure'
    data: TableData | null
    filterCount: number
    filterOpen?: boolean
    loading: boolean
    hiddenColumns: string[]
    ontogglecolumn: (column: string) => void
    onshowallcolumns: () => void
    exportHref: (format: 'csv' | 'json') => string
    onreload: () => void
    oninsert: () => void
    viewer: Viewer
    viewerLabels: RunAsLabels
    onviewer: (viewer: Viewer) => void
  } = $props()

  const tabs = [
    { view: 'data', label: 'tables.toolbar.data', suffix: '' },
    { view: 'structure', label: 'tables.toolbar.structure', suffix: '/structure' },
  ] as const
</script>

<div class="flex min-h-14 shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b bg-background px-4 py-2.5">
  <a
    href={href('/tables')}
    class="-ml-1 grid size-9 place-items-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground lg:hidden"
    aria-label={t('tables.toolbar.back')}><ChevronLeft class="size-4" /></a
  >
  <h1 class="min-w-0 truncate text-base font-semibold">{name}</h1>
  {#if data}<RlsBadge rls={data.table.rls} />{/if}
  <nav class="ml-1 flex h-9 items-center gap-0.5 rounded-md border bg-muted/50 p-0.5 text-sm" aria-label={t('tables.toolbar.views')}>
    {#each tabs as tab (tab.view)}
      <a
        href={href(`/tables/${enc(name)}${tab.suffix}${route.query.size ? `?${route.query}` : ''}`)}
        aria-current={view === tab.view ? 'page' : undefined}
        class={[
          'flex h-full items-center rounded px-3 transition-colors',
          view === tab.view ? 'bg-background font-medium text-foreground ring-1 ring-border' : 'text-muted-foreground hover:text-foreground',
        ]}>{t(tab.label)}</a
      >
    {/each}
  </nav>
  {#if view === 'data'}
    <div class="ml-auto flex flex-wrap items-center gap-2">
      <RunAsPicker {viewer} labels={viewerLabels} onchange={onviewer} />
      <Button
        variant={filterOpen || filterCount ? 'secondary' : 'ghost'}
        size="sm"
        onclick={() => (filterOpen = !filterOpen)}
        aria-expanded={filterOpen}
      >
        <Funnel />{t('tables.toolbar.filter')}{#if filterCount}<span class="text-muted-foreground tabular-nums">{filterCount}</span>{/if}
      </Button>
      {#if data}
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button variant={hiddenColumns.length ? 'secondary' : 'ghost'} size="sm" {...props}>
                <Columns3 />{t('tables.toolbar.columns')}{#if hiddenColumns.length}<span
                    class="text-muted-foreground tabular-nums"
                    title={t('tables.toolbar.hidden', { count: hiddenColumns.length })}>{hiddenColumns.length}</span
                  >{/if}
              </Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="max-h-96 w-64 overflow-y-auto">
            <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">{t('tables.toolbar.visibleColumns')}</DropdownMenu.Label>
            {#each data.table.columns as column (column.name)}
              <DropdownMenu.CheckboxItem
                checked={!hiddenColumns.includes(column.name)}
                closeOnSelect={false}
                onCheckedChange={() => ontogglecolumn(column.name)}
                class="font-mono text-xs">{column.name}</DropdownMenu.CheckboxItem
              >
            {/each}
            {#if hiddenColumns.length}
              <DropdownMenu.Separator />
              <DropdownMenu.Item onclick={onshowallcolumns}>{t('tables.toolbar.showAll')}</DropdownMenu.Item>
            {/if}
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      {/if}
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button variant="ghost" size="sm" {...props}><Download />{t('tables.toolbar.export')}</Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end" class="w-56">
          <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">
            {filterCount ? t('tables.toolbar.exportFiltered') : t('tables.toolbar.exportAll')}
          </DropdownMenu.Label>
          <DropdownMenu.Item>
            {#snippet child({ props })}<a {...props} href={exportHref('csv')} download>CSV</a>{/snippet}
          </DropdownMenu.Item>
          <DropdownMenu.Item>
            {#snippet child({ props })}<a {...props} href={exportHref('json')} download>JSON</a>{/snippet}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
      <Button variant="ghost" size="icon-sm" disabled={loading} onclick={onreload} aria-label={t('tables.toolbar.reload')} title={t('tables.toolbar.reload')}>
        <RefreshCw class={loading ? 'animate-spin' : ''} />
      </Button>
      {#if data?.table.insertable}
        <Button size="sm" disabled={loading} onclick={oninsert}><Plus />{t('tables.toolbar.insertRow')}</Button>
      {/if}
    </div>
  {/if}
</div>
