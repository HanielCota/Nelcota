<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import * as Select from '$lib/components/ui/select'
  import ChevronLeft from '@lucide/svelte/icons/chevron-left'
  import ChevronRight from '@lucide/svelte/icons/chevron-right'
  import ChevronsLeft from '@lucide/svelte/icons/chevrons-left'
  import ChevronsRight from '@lucide/svelte/icons/chevrons-right'
  import { pageInfo } from '$lib/grid'
  import type { TableData } from '$lib/types'

  let {
    data,
    page = $bindable(),
    size = $bindable(),
  }: { data: TableData; page: number; size: string } = $props()

  const fmt = new Intl.NumberFormat('pt-BR')
  const info = $derived(pageInfo(data.page, data.size, data.rows.length, data.total, data.total_exact))
  const totalLabel = $derived(
    data.total === null ? null : `${data.total_exact ? '' : '~'}${fmt.format(data.total)}`,
  )
</script>

<footer class="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 border-t bg-sidebar px-4 py-2 text-xs text-muted-foreground">
  <span class="tabular-nums" aria-live="polite">
    {#if info.to === 0}Nenhuma linha
    {:else}<span class="text-foreground">{fmt.format(info.from)}–{fmt.format(info.to)}</span>
      {#if totalLabel}de {totalLabel}{/if}
      {data.total === 1 ? 'linha' : 'linhas'}{/if}
  </span>
  <span class="hidden xl:inline">
    Setas navegam{#if data.table.editable}, Enter ou duplo clique edita{/if}, Ctrl+C copia.
  </span>
  <div class="ml-auto flex items-center gap-1.5">
    <span class="mr-1 hidden sm:inline">Por página</span>
    <Select.Root type="single" bind:value={size} onValueChange={() => (page = 0)}>
      <Select.Trigger size="sm" class="h-7 w-20" aria-label="Linhas por página">{size}</Select.Trigger>
      <Select.Content>
        {#each ['25', '50', '100', '500'] as option (option)}
          <Select.Item value={option}>{option}</Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
    <span class="px-2 tabular-nums">
      Página {fmt.format(data.page + 1)}{#if info.pageCount}{` de ${fmt.format(info.pageCount)}`}{/if}
    </span>
    <Button variant="outline" size="icon-sm" disabled={page === 0} onclick={() => (page = 0)} aria-label="Primeira página">
      <ChevronsLeft />
    </Button>
    <Button variant="outline" size="icon-sm" disabled={page === 0} onclick={() => page--} aria-label="Página anterior">
      <ChevronLeft />
    </Button>
    <Button variant="outline" size="icon-sm" disabled={!data.has_next} onclick={() => page++} aria-label="Próxima página">
      <ChevronRight />
    </Button>
    <Button
      variant="outline"
      size="icon-sm"
      disabled={info.lastPage === null || page >= info.lastPage}
      onclick={() => info.lastPage !== null && (page = info.lastPage)}
      aria-label="Última página"
      title={info.lastPage === null ? 'Total estimado: a última página não é conhecida' : undefined}
    >
      <ChevronsRight />
    </Button>
  </div>
</footer>
