<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import Search from '@lucide/svelte/icons/search'
  import Plus from '@lucide/svelte/icons/plus'
  import Table2 from '@lucide/svelte/icons/table-2'
  import RlsDot from './RlsDot.svelte'
  import { href } from '$lib/router.svelte'
  import { cn } from '$lib/utils'
  import type { TableSummary } from '$lib/types'
  import { t } from '$lib/i18n/index.svelte'

  let {
    tables,
    current,
    oncreate,
  }: {
    tables: TableSummary[]
    /** Open table (`undefined` = none: on phones the list fills the screen). */
    current?: string
    oncreate: () => void
  } = $props()

  let search = $state('')
  const visible = $derived(tables.filter((table) => table.name.toLowerCase().includes(search.trim().toLowerCase())))

  // Tooltip from the RLS state (the server's label is English).
  function rlsTitle(rls: TableSummary['rls']): string {
    if (rls.state === 'ok') return t('tables.sidebar.rls.ok', { count: rls.policies })
    return t(`tables.sidebar.rls.${rls.state}`)
  }
</script>

<aside
  aria-label={t('tables.sidebar.label')}
  class={cn('w-full shrink-0 flex-col border-r bg-sidebar md:flex md:w-72', current ? 'hidden' : 'flex')}
>
  <div class="grid gap-3 border-b p-4">
    <div class="flex items-center justify-between gap-2">
      <svelte:element this={current ? 'p' : 'h1'} class="px-1 text-sm font-semibold">{t('tables.sidebar.title')}</svelte:element>
      <Button variant="outline" size="sm" title={t('tables.sidebar.newTable')} aria-label={t('tables.sidebar.newTable')} onclick={oncreate}>
        <Plus />{t('tables.sidebar.new')}
      </Button>
    </div>
    <div class="relative">
      <Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
      <Input bind:value={search} placeholder={t('tables.sidebar.search')} class="pl-9 text-sm" />
    </div>
  </div>
  <p class="flex items-center justify-between px-5 pt-4 pb-2 text-xs font-medium text-muted-foreground">
    {t('tables.sidebar.heading')}<span class="tabular-nums">{visible.length}</span>
  </p>
  <nav class="flex flex-1 flex-col gap-0.5 overflow-y-auto px-3 pb-3" aria-label={t('tables.sidebar.heading')}>
    {#each visible as table (table.name)}
      <a
        href={href(`/tables/${encodeURIComponent(table.name)}`)}
        title={rlsTitle(table.rls)}
        aria-current={table.name === current ? 'page' : undefined}
        class={cn(
          'flex h-11 items-center gap-2.5 rounded-md px-3 text-sm text-muted-foreground transition-colors hover:bg-accent/60 hover:text-foreground md:h-9',
          table.name === current && 'bg-accent font-medium text-foreground',
        )}
      >
        <Table2 class={['size-4 shrink-0', table.name === current && 'text-brand']} strokeWidth={1.6} />
        <span class="truncate">{table.name}</span>
        <span class="ml-auto flex"><RlsDot state={table.rls.state} /></span>
        {#if table.kind !== 'table'}<span class="text-2xs text-muted-foreground">{t('tables.sidebar.view')}</span>{/if}
      </a>
    {:else}
      <p class="px-3 py-6 text-center text-sm text-muted-foreground">{t('tables.sidebar.empty')}</p>
    {/each}
  </nav>
</aside>
