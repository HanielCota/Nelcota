<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import Plus from '@lucide/svelte/icons/plus'
  import { SQL_SNIPPETS } from '$lib/sql-snippets'
  import { sqlStore, type SavedQuery } from '$lib/sql-store.svelte'
  import { cn } from '$lib/utils'

  let {
    onrename,
    ondelete,
  }: {
    onrename: (query: SavedQuery) => void
    ondelete: (query: SavedQuery) => void
  } = $props()

  const item =
    'group flex h-9 w-full cursor-pointer items-center gap-2 rounded-md px-2.5 text-left text-sm text-muted-foreground transition-colors hover:bg-accent/60 hover:text-foreground'
  const heading = 'px-2.5 pb-1 text-xs font-medium text-muted-foreground'
</script>

<aside class="hidden w-72 shrink-0 flex-col border-r bg-sidebar lg:flex" aria-label="Consultas salvas e modelos">
  <div class="flex h-14 items-center justify-between border-b pr-2 pl-4">
    <p class="text-sm font-semibold">Editor SQL</p>
    <Button variant="ghost" size="icon-sm" onclick={() => sqlStore.open('')} title="Nova consulta" aria-label="Nova consulta">
      <Plus />
    </Button>
  </div>

  <div class="flex-1 overflow-y-auto px-2 py-3">
    <p class={heading}>
      Salvas <span class="tabular-nums">({sqlStore.saved.length})</span>
    </p>
    <div class="grid gap-0.5">
      {#each sqlStore.sorted as query (query.id)}
        {@const active = query.id === sqlStore.currentId}
        <div class={cn(item, 'pr-1', active && 'bg-sidebar-accent font-medium text-foreground')}>
          <button
            class="flex h-full min-w-0 flex-1 cursor-pointer items-center gap-2"
            aria-current={active ? 'true' : undefined}
            onclick={() => sqlStore.openSaved(query.id)}
          >
            <span class="truncate">{query.name}</span>
            {#if active && sqlStore.dirty}<span class="size-1.5 shrink-0 rounded-full bg-muted-foreground" title="Alterações não salvas"></span>{/if}
          </button>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger>
              {#snippet child({ props })}
                <button
                  {...props}
                  class="grid size-7 shrink-0 cursor-pointer place-items-center rounded-md opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover:bg-accent aria-expanded:opacity-100"
                  aria-label={`Ações de ${query.name}`}
                >
                  <Ellipsis class="size-4" />
                </button>
              {/snippet}
            </DropdownMenu.Trigger>
            <DropdownMenu.Content align="start" class="w-44">
              <DropdownMenu.Item onclick={() => onrename(query)}>Renomear</DropdownMenu.Item>
              <DropdownMenu.Separator />
              <DropdownMenu.Item variant="destructive" onclick={() => ondelete(query)}>Apagar</DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </div>
      {:else}
        <p class="px-2.5 py-1 text-xs text-muted-foreground">Nenhuma consulta salva.</p>
      {/each}
    </div>

    <p class={cn(heading, 'mt-6')}>Modelos</p>
    <div class="grid gap-0.5">
      {#each SQL_SNIPPETS as snippet (snippet.label)}
        <button class={item} onclick={() => sqlStore.open(snippet.sql)}>
          <span class="truncate">{snippet.label}</span>
        </button>
      {/each}
    </div>
  </div>
</aside>
