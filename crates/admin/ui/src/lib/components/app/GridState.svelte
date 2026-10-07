<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Plus from '@lucide/svelte/icons/plus'
  import FunnelX from '@lucide/svelte/icons/funnel-x'
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert'
  import Inbox from '@lucide/svelte/icons/inbox'

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
    <div class="flex gap-3 border-b bg-card px-4 py-3">
      {#each [1, 2, 3, 4, 5] as i (i)}<Skeleton class="h-6 w-32" />{/each}
    </div>
    {#each Array.from({ length: 10 }, (_, i) => i) as i (i)}
      <div class="flex gap-3 border-b px-4 py-2.5">
        {#each [1, 2, 3, 4, 5] as j (j)}<Skeleton class={['h-4', j === 1 ? 'w-12' : 'w-32']} />{/each}
      </div>
    {/each}
  </div>
{:else}
  <div class="grid place-items-center px-6 py-16 text-center">
    <div class="max-w-sm">
      {#if state === 'error'}
        <TriangleAlert class="mx-auto size-7 text-destructive" strokeWidth={1.4} />
        <p class="mt-3 text-sm font-medium">Não deu para carregar as linhas</p>
        <p class="mt-1 text-sm font-light break-words text-muted-foreground">{message}</p>
        {#if onretry}<Button variant="outline" size="sm" class="mt-4" onclick={onretry}>Tentar de novo</Button>{/if}
      {:else if state === 'no-match'}
        <FunnelX class="mx-auto size-7 text-muted-foreground" strokeWidth={1.4} />
        <p class="mt-3 text-sm font-medium">Nenhuma linha para esses filtros</p>
        <p class="mt-1 text-sm font-light text-muted-foreground">Ajuste ou remova os filtros para ver mais linhas.</p>
        {#if onclearfilters}<Button variant="outline" size="sm" class="mt-4" onclick={onclearfilters}>Limpar filtros</Button>{/if}
      {:else}
        <Inbox class="mx-auto size-7 text-muted-foreground" strokeWidth={1.4} />
        <p class="mt-3 text-sm font-medium">Esta tabela está vazia</p>
        <p class="mt-1 text-sm font-light text-muted-foreground">
          {insertable ? 'Insira a primeira linha aqui ou pela API.' : 'As linhas aparecem aqui quando forem criadas.'}
        </p>
        {#if insertable && oninsert}<Button size="sm" class="mt-4" onclick={oninsert}><Plus />Inserir linha</Button>{/if}
      {/if}
    </div>
  </div>
{/if}
