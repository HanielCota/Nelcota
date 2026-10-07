<script lang="ts">
  import ArrowUp from '@lucide/svelte/icons/arrow-up'
  import ArrowDown from '@lucide/svelte/icons/arrow-down'
  import ArrowUpDown from '@lucide/svelte/icons/arrow-up-down'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import { alignRight, type ColumnKind } from '$lib/grid'
  import type { Column } from '$lib/types'

  let {
    column,
    kind,
    sort,
    onsort,
  }: {
    column: Column
    kind: ColumnKind
    /** `asc`/`desc` se a grade está ordenada por esta coluna. */
    sort: 'asc' | 'desc' | null
    onsort: () => void
  } = $props()

  const sortLabel = $derived(
    sort === 'asc' ? 'ordenado do menor para o maior' : sort === 'desc' ? 'ordenado do maior para o menor' : 'sem ordenação',
  )
</script>

<button
  type="button"
  class={[
    'group/head flex h-full w-full items-start gap-1.5 px-3 py-2 text-left transition-colors hover:bg-accent',
    alignRight(kind) && 'flex-row-reverse text-right',
  ]}
  onclick={onsort}
  title={column.comment ?? undefined}
  aria-label={`${column.name}, ${column.full_type}, ${sortLabel}. Clique para ordenar.`}
>
  <span class="grid min-w-0 flex-1">
    <span class={['flex items-center gap-1 truncate text-xs font-medium text-foreground', alignRight(kind) && 'justify-end']}>
      {#if column.is_pk}<KeyRound class="size-3 shrink-0 text-brand" aria-hidden="true" />{/if}
      <span class="truncate">{column.name}</span>
    </span>
    <span class="truncate font-mono text-3xs font-normal text-muted-foreground"
      >{column.full_type}{#if column.references}<span class="text-brand">{` → ${column.references.table}`}</span>{/if}</span
    >
  </span>
  <!-- Indicador de ordenação: discreto até passar o mouse; em destaque quando ativo. -->
  <span class="mt-0.5 shrink-0" aria-hidden="true">
    {#if sort === 'asc'}<ArrowUp class="size-3.5 text-brand" />
    {:else if sort === 'desc'}<ArrowDown class="size-3.5 text-brand" />
    {:else}<ArrowUpDown class="size-3.5 text-muted-foreground opacity-0 transition-opacity group-hover/head:opacity-100" />{/if}
  </span>
</button>
