<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import RefreshCw from '@lucide/svelte/icons/refresh-cw'
  import Plus from '@lucide/svelte/icons/plus'
  import ChevronLeft from '@lucide/svelte/icons/chevron-left'
  import Funnel from '@lucide/svelte/icons/funnel'
  import Download from '@lucide/svelte/icons/download'
  import Columns3 from '@lucide/svelte/icons/columns-3'
  import RlsBadge from './RlsBadge.svelte'
  import { enc } from '$lib/api'
  import { href } from '$lib/router.svelte'
  import type { TableData } from '$lib/types'

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
  } = $props()

  const tabs = [
    { view: 'data', label: 'Dados', suffix: '' },
    { view: 'structure', label: 'Estrutura', suffix: '/structure' },
  ] as const
</script>

<div class="flex min-h-12 shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b px-4 py-2">
  <a
    href={href('/tables')}
    class="-ml-1 grid size-7 place-items-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground md:hidden"
    aria-label="Voltar para a lista de tabelas"><ChevronLeft class="size-4" /></a
  >
  <h1 class="min-w-0 truncate text-sm font-medium">{name}</h1>
  {#if data}<RlsBadge rls={data.table.rls} />{/if}
  <nav class="ml-1 flex items-center gap-0.5 rounded-md bg-muted p-0.5 text-xs" aria-label="Visão da tabela">
    {#each tabs as tab (tab.view)}
      <a
        href={href(`/tables/${enc(name)}${tab.suffix}`)}
        aria-current={view === tab.view ? 'page' : undefined}
        class={[
          'rounded px-2.5 py-1 transition-colors',
          view === tab.view ? 'bg-card text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground',
        ]}>{tab.label}</a
      >
    {/each}
  </nav>
  {#if view === 'data'}
    <div class="ml-auto flex flex-wrap items-center gap-2">
      <Button
        variant={filterOpen || filterCount ? 'secondary' : 'ghost'}
        size="sm"
        onclick={() => (filterOpen = !filterOpen)}
        aria-expanded={filterOpen}
      >
        <Funnel />Filtrar{#if filterCount}<span class="rounded-full bg-brand/15 px-1.5 text-3xs text-brand tabular-nums"
            >{filterCount}</span
          >{/if}
      </Button>
      {#if data}
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button variant={hiddenColumns.length ? 'secondary' : 'ghost'} size="sm" {...props}>
                <Columns3 />Colunas{#if hiddenColumns.length}<span
                    class="rounded-full bg-brand/15 px-1.5 text-3xs text-brand tabular-nums"
                    title={`${hiddenColumns.length} oculta(s)`}>{hiddenColumns.length}</span
                  >{/if}
              </Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="max-h-80 w-56 overflow-y-auto">
            <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">Colunas visíveis</DropdownMenu.Label>
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
              <DropdownMenu.Item onclick={onshowallcolumns}>Mostrar todas</DropdownMenu.Item>
            {/if}
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      {/if}
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button variant="ghost" size="sm" {...props}><Download />Exportar</Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end" class="w-56">
          <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">
            {filterCount ? 'Linhas filtradas, na ordem atual' : 'Todas as linhas, na ordem atual'}
          </DropdownMenu.Label>
          <DropdownMenu.Item>
            {#snippet child({ props })}<a {...props} href={exportHref('csv')} download>CSV</a>{/snippet}
          </DropdownMenu.Item>
          <DropdownMenu.Item>
            {#snippet child({ props })}<a {...props} href={exportHref('json')} download>JSON</a>{/snippet}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
      <Button variant="ghost" size="icon-sm" onclick={onreload} aria-label="Recarregar" title="Recarregar">
        <RefreshCw class={loading ? 'animate-spin' : ''} />
      </Button>
      {#if data?.table.insertable}
        <Button size="sm" onclick={oninsert}><Plus />Inserir linha</Button>
      {/if}
    </div>
  {/if}
</div>
