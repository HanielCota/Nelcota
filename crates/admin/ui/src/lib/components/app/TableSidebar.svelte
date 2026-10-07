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

  let {
    tables,
    current,
    oncreate,
  }: {
    tables: TableSummary[]
    /** Tabela aberta (`undefined` = nenhuma: no celular a lista ocupa a tela). */
    current?: string
    oncreate: () => void
  } = $props()

  let search = $state('')
  const visible = $derived(tables.filter((t) => t.name.toLowerCase().includes(search.trim().toLowerCase())))
</script>

<aside
  aria-label="Lista de tabelas"
  class={cn('w-full shrink-0 flex-col border-r bg-sidebar md:flex md:w-72', current ? 'hidden' : 'flex')}
>
  <div class="grid gap-3 border-b p-4">
    <div class="flex items-center justify-between gap-2">
      <svelte:element this={current ? 'p' : 'h1'} class="px-1 text-base font-semibold tracking-tight">Editor de tabelas</svelte:element>
      <Button variant="outline" size="sm" title="Nova tabela" aria-label="Nova tabela" onclick={oncreate}>
        <Plus />Nova
      </Button>
    </div>
    <div class="relative">
      <Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
      <Input bind:value={search} placeholder="Buscar tabelas…" class="pl-9 text-sm" />
    </div>
  </div>
  <p class="flex items-center justify-between px-5 pt-4 pb-2 text-3xs font-semibold tracking-[0.08em] text-muted-foreground uppercase">
    Tabelas<span class="rounded-full bg-muted px-2 py-0.5 tabular-nums">{visible.length}</span>
  </p>
  <nav class="flex flex-1 flex-col gap-0.5 overflow-y-auto px-3 pb-3" aria-label="Tabelas">
    {#each visible as table (table.name)}
      <a
        href={href(`/tables/${encodeURIComponent(table.name)}`)}
        title={table.rls.label}
        aria-current={table.name === current ? 'page' : undefined}
        class={cn(
          'relative flex h-11 items-center gap-2.5 rounded-lg px-3 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent/60 hover:text-foreground md:h-9',
          table.name === current && 'bg-sidebar-accent text-foreground shadow-card',
        )}
      >
        {#if table.name === current}<span class="absolute inset-y-2 left-0 w-[3px] rounded-r-full bg-brand" aria-hidden="true"></span>{/if}
        <Table2 class={['size-4 shrink-0', table.name === current && 'text-brand']} strokeWidth={1.75} />
        <span class="truncate">{table.name}</span>
        <span class="ml-auto flex"><RlsDot state={table.rls.state} /></span>
        {#if table.kind !== 'table'}<span class="rounded-md bg-muted px-1.5 py-0.5 text-3xs font-semibold">view</span>{/if}
      </a>
    {:else}
      <p class="px-3 py-6 text-center text-sm text-muted-foreground">Nenhuma tabela.</p>
    {/each}
  </nav>
</aside>
