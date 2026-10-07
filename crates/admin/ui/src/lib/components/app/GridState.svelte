<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Plus from '@lucide/svelte/icons/plus'
  import FunnelX from '@lucide/svelte/icons/funnel-x'
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert'
  import Inbox from '@lucide/svelte/icons/inbox'
  import RotateCw from '@lucide/svelte/icons/rotate-cw'
  import EmptyState from './EmptyState.svelte'

  // Estados da grade sem linhas para mostrar: carregando pela primeira vez,
  // erro, filtro sem resultado e tabela vazia.
  let {
    state,
    message = '',
    insertable = false,
    onretry,
    onclearfilters,
    oninsert,
  }: {
    state: 'loading' | 'error' | 'no-match' | 'empty'
    message?: string
    insertable?: boolean
    onretry?: () => void
    onclearfilters?: () => void
    oninsert?: () => void
  } = $props()
</script>

{#if state === 'loading'}
  <div class="grid gap-px p-0" aria-busy="true" aria-label="Carregando linhas">
    <div class="flex gap-3 border-b bg-card px-4 py-3.5">
      {#each [1, 2, 3, 4, 5] as i (i)}<Skeleton class="h-6 w-32" />{/each}
    </div>
    {#each Array.from({ length: 10 }, (_, i) => i) as i (i)}
      <div class="flex gap-3 border-b px-4 py-3">
        {#each [1, 2, 3, 4, 5] as j (j)}<Skeleton class={['h-4', j === 1 ? 'w-12' : 'w-32']} />{/each}
      </div>
    {/each}
  </div>
{:else if state === 'error'}
  <EmptyState icon={TriangleAlert} title="Não deu para carregar as linhas" class="m-6 border-destructive/30 bg-destructive/5 [&>span:first-child]:text-destructive">
    <span class="break-words">{message}</span>
    {#snippet actions()}
      {#if onretry}<Button variant="outline" onclick={onretry}><RotateCw />Tentar de novo</Button>{/if}
    {/snippet}
  </EmptyState>
{:else if state === 'no-match'}
  <EmptyState
    icon={FunnelX}
    title="Nenhuma linha para esses filtros"
    description="Ajuste ou remova os filtros para ver mais linhas."
    class="m-6"
  >
    {#snippet actions()}
      {#if onclearfilters}<Button variant="outline" onclick={onclearfilters}>Limpar filtros</Button>{/if}
    {/snippet}
  </EmptyState>
{:else}
  <EmptyState
    icon={Inbox}
    title="Esta tabela está vazia"
    description={insertable ? 'Insira a primeira linha aqui ou pela API.' : 'As linhas aparecem aqui quando forem criadas.'}
    class="m-6"
  >
    {#snippet actions()}
      {#if insertable && oninsert}<Button onclick={oninsert}><Plus />Inserir linha</Button>{/if}
    {/snippet}
  </EmptyState>
{/if}
