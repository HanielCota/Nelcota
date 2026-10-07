<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import * as Select from '$lib/components/ui/select'
  import ChevronLeft from '@lucide/svelte/icons/chevron-left'
  import ChevronRight from '@lucide/svelte/icons/chevron-right'
  import type { TableData } from '$lib/types'

  let {
    data,
    page = $bindable(),
    size = $bindable(),
  }: { data: TableData; page: number; size: string } = $props()

  const fmt = new Intl.NumberFormat('pt-BR')
</script>

<footer class="flex shrink-0 flex-wrap items-center gap-3 border-t bg-sidebar px-4 py-2 text-xs text-muted-foreground">
  <span>
    {#if data.total !== null}{data.total_exact ? '' : '~'}{fmt.format(data.total)}
      {data.total === 1 ? 'linha' : 'linhas'}{:else}{data.rows.length} nesta página{/if}
  </span>
  {#if data.table.editable}<span class="hidden lg:inline">Duplo clique numa célula para editar.</span>{/if}
  <div class="ml-auto flex items-center gap-2">
    <span>Por página</span>
    <Select.Root type="single" bind:value={size} onValueChange={() => (page = 0)}>
      <Select.Trigger size="sm" class="h-7 w-20">{size}</Select.Trigger>
      <Select.Content>
        {#each ['25', '50', '100', '500'] as option (option)}
          <Select.Item value={option}>{option}</Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
    <span class="px-1">Página {data.page + 1}</span>
    <Button variant="outline" size="icon-sm" disabled={page === 0} onclick={() => page--} aria-label="Anterior">
      <ChevronLeft />
    </Button>
    <Button variant="outline" size="icon-sm" disabled={!data.has_next} onclick={() => page++} aria-label="Próxima">
      <ChevronRight />
    </Button>
  </div>
</footer>
