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
  class={cn('w-full shrink-0 flex-col border-r bg-sidebar md:flex md:w-64', current ? 'hidden' : 'flex')}
>
  <div class="grid gap-3 border-b p-3">
    <div class="flex items-center justify-between">
      <svelte:element this={current ? 'p' : 'h1'} class="px-1 text-sm font-medium">Editor de tabelas</svelte:element>
      <Button variant="ghost" size="icon-sm" title="Nova tabela" aria-label="Nova tabela" onclick={oncreate}>
        <Plus />
      </Button>
    </div>
    <div class="relative">
      <Search class="absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input bind:value={search} placeholder="Buscar tabelas…" class="h-8 bg-card pl-8 text-sm" />
    </div>
  </div>
  <p class="px-4 pt-3 pb-1 text-2xs font-medium tracking-wider text-muted-foreground uppercase">
    Tabelas ({visible.length})
  </p>
  <nav class="flex-1 overflow-y-auto px-2 pb-2" aria-label="Tabelas">
    {#each visible as table (table.name)}
      <a
        href={href(`/tables/${encodeURIComponent(table.name)}`)}
        title={table.rls.label}
        aria-current={table.name === current ? 'page' : undefined}
        class={cn(
          'flex h-10 items-center gap-2 rounded-md px-2 text-sm text-muted-foreground transition-colors hover:bg-accent/60 hover:text-foreground md:h-8',
          table.name === current && 'bg-accent text-foreground',
        )}
      >
        <Table2 class={['size-3.5 shrink-0', table.name === current && 'text-brand']} strokeWidth={1.6} />
        <span class="truncate">{table.name}</span>
        <span class="ml-auto flex"><RlsDot state={table.rls.state} /></span>
        {#if table.kind !== 'table'}<span class="text-2xs">view</span>{/if}
      </a>
    {:else}
      <p class="px-2 py-4 text-sm text-muted-foreground">Nenhuma tabela.</p>
    {/each}
  </nav>
</aside>
