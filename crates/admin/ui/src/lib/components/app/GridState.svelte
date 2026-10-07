<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Plus from '@lucide/svelte/icons/plus'
  import RotateCw from '@lucide/svelte/icons/rotate-cw'
  import EmptyState from './EmptyState.svelte'
  import { t } from '$lib/i18n/index.svelte'

  // Grid states without rows to show: first load, error, a filter without
  // results and an empty table.
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
  <div class="grid gap-px p-0" aria-busy="true" aria-label={t('tables.state.loading')}>
    <div class="flex gap-3 border-b px-4 py-3.5">
      {#each [1, 2, 3, 4, 5] as i (i)}<Skeleton class="h-6 w-32" />{/each}
    </div>
    {#each Array.from({ length: 10 }, (_, i) => i) as i (i)}
      <div class="flex gap-3 border-b px-4 py-3">
        {#each [1, 2, 3, 4, 5] as j (j)}<Skeleton class={['h-4', j === 1 ? 'w-12' : 'w-32']} />{/each}
      </div>
    {/each}
  </div>
{:else if state === 'error'}
  <div class="grid justify-items-start gap-3 p-6 text-sm">
    <p class="break-words text-destructive">{t('tables.state.loadError', { message })}</p>
    {#if onretry}<Button variant="outline" size="sm" onclick={onretry}><RotateCw />{t('common.retry')}</Button>{/if}
  </div>
{:else if state === 'no-match'}
  <EmptyState title={t('tables.state.noMatch')}>
    {#snippet actions()}
      {#if onclearfilters}<Button variant="outline" onclick={onclearfilters}>{t('tables.state.clearFilters')}</Button>{/if}
    {/snippet}
  </EmptyState>
{:else}
  <EmptyState title={t('tables.state.empty')}>
    {#snippet actions()}
      {#if insertable && oninsert}<Button variant="outline" onclick={oninsert}><Plus />{t('tables.state.insertRow')}</Button>{/if}
    {/snippet}
  </EmptyState>
{/if}
