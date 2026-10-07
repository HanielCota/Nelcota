<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import ArrowUp from '@lucide/svelte/icons/arrow-up'
  import ArrowDown from '@lucide/svelte/icons/arrow-down'
  import ArrowUpDown from '@lucide/svelte/icons/arrow-up-down'
  import ChevronDown from '@lucide/svelte/icons/chevron-down'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import Funnel from '@lucide/svelte/icons/funnel'
  import Copy from '@lucide/svelte/icons/copy'
  import EyeOff from '@lucide/svelte/icons/eye-off'
  import X from '@lucide/svelte/icons/x'
  import { toast } from 'svelte-sonner'
  import { alignRight, type ColumnKind } from '$lib/grid'
  import type { Column } from '$lib/types'

  let {
    column,
    kind,
    sort,
    onsort,
    onsortset,
    onfilter,
    onhide,
  }: {
    column: Column
    kind: ColumnKind
    /** `asc`/`desc` se a grade está ordenada por esta coluna. */
    sort: 'asc' | 'desc' | null
    /** Clique no nome: alterna crescente → decrescente → sem ordem. */
    onsort: () => void
    onsortset: (direction: 'asc' | 'desc' | null) => void
    onfilter: () => void
    onhide: () => void
  } = $props()

  const sortLabel = $derived(
    sort === 'asc' ? 'ordenado do menor para o maior' : sort === 'desc' ? 'ordenado do maior para o menor' : 'sem ordenação',
  )

  // Ações que recarregam a grade rodam só depois que o menu termina de fechar:
  // re-renderizar o cabeçalho no meio da animação de fechamento fazia o bits-ui
  // perder a limpeza e deixava `pointer-events: none` no body (página travada).
  let pending: (() => void) | null = null
  const afterClose = (action: () => void) => () => (pending = action)

  function onOpenChangeComplete(open: boolean) {
    if (open || !pending) return
    const action = pending
    pending = null
    action()
  }

  async function copyName() {
    await navigator.clipboard.writeText(column.name)
    toast.success(`"${column.name}" copiado`)
  }
</script>

<div class="group/head flex h-full items-stretch">
  <button
    type="button"
    class={[
      'flex min-w-0 flex-1 cursor-pointer items-start gap-1.5 py-2.5 pl-3.5 text-left transition-colors hover:bg-accent/70',
      alignRight(kind) && 'flex-row-reverse text-right',
    ]}
    onclick={onsort}
    title={column.comment ?? undefined}
    aria-label={`${column.name}, ${column.full_type}, ${sortLabel}. Clique para ordenar.`}
  >
    <span class="grid min-w-0 flex-1">
      <span class={['flex items-center gap-1.5 truncate text-xs font-semibold text-foreground', alignRight(kind) && 'justify-end']}>
        {#if column.is_pk}<KeyRound class="size-3.5 shrink-0 text-brand" aria-hidden="true" />{/if}
        <span class="truncate">{column.name}</span>
      </span>
      <span class="mt-0.5 truncate font-mono text-3xs font-normal text-muted-foreground"
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
  <DropdownMenu.Root {onOpenChangeComplete}>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <button
          {...props}
          type="button"
          class="grid w-7 shrink-0 cursor-pointer place-items-center text-muted-foreground transition-opacity hover:bg-accent hover:text-foreground focus-visible:opacity-100 aria-expanded:bg-accent aria-expanded:opacity-100 md:opacity-0 md:group-hover/head:opacity-100"
          aria-label={`Opções da coluna ${column.name}`}
        >
          <ChevronDown class="size-3.5" />
        </button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="end" class="w-56">
      <DropdownMenu.Label class="truncate font-mono text-xs font-normal text-muted-foreground">{column.name}</DropdownMenu.Label>
      <DropdownMenu.Item onclick={afterClose(() => onsortset('asc'))} disabled={sort === 'asc'}
        ><ArrowUp />Ordenar crescente</DropdownMenu.Item
      >
      <DropdownMenu.Item onclick={afterClose(() => onsortset('desc'))} disabled={sort === 'desc'}
        ><ArrowDown />Ordenar decrescente</DropdownMenu.Item
      >
      {#if sort}<DropdownMenu.Item onclick={afterClose(() => onsortset(null))}><X />Remover ordenação</DropdownMenu.Item>{/if}
      <DropdownMenu.Separator />
      <DropdownMenu.Item onclick={afterClose(onfilter)}><Funnel />Filtrar por esta coluna</DropdownMenu.Item>
      <DropdownMenu.Item onclick={copyName}><Copy />Copiar nome</DropdownMenu.Item>
      <DropdownMenu.Item onclick={afterClose(onhide)}><EyeOff />Ocultar coluna</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
</div>
