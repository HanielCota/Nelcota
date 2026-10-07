<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import FileCode from '@lucide/svelte/icons/file-code'
  import Sparkles from '@lucide/svelte/icons/sparkles'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import FilePlus from '@lucide/svelte/icons/file-plus'
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
    'group flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-sm text-muted-foreground transition-colors hover:bg-accent/60 hover:text-foreground'
</script>

<aside class="hidden w-60 shrink-0 flex-col border-r bg-sidebar lg:flex">
  <div class="flex h-12 items-center justify-between border-b px-4">
    <p class="text-sm font-medium">Editor SQL</p>
    <button
      class="grid size-7 place-items-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
      title="Nova consulta"
      aria-label="Nova consulta"
      onclick={() => sqlStore.open('')}
    >
      <FilePlus class="size-4" />
    </button>
  </div>

  <div class="flex-1 overflow-y-auto p-2">
    <p class="px-2 pt-1 pb-1 text-2xs font-medium tracking-wider text-muted-foreground uppercase">
      Salvas ({sqlStore.saved.length})
    </p>
    {#each sqlStore.sorted as query (query.id)}
      {@const active = query.id === sqlStore.currentId}
      <div class={cn(item, 'pr-1', active && 'bg-accent text-foreground')}>
        <button class="flex min-w-0 flex-1 items-center gap-2" onclick={() => sqlStore.openSaved(query.id)}>
          <FileCode class={['size-3.5 shrink-0', active && 'text-brand']} strokeWidth={1.6} />
          <span class="truncate">{query.name}</span>
          {#if active && sqlStore.dirty}<span class="size-1.5 shrink-0 rounded-full bg-warning" title="Alterações não salvas"></span>{/if}
        </button>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <button
                {...props}
                class="grid size-6 shrink-0 place-items-center rounded opacity-0 group-hover:opacity-100 hover:bg-accent aria-expanded:opacity-100"
                aria-label={`Ações de ${query.name}`}
              >
                <Ellipsis class="size-3.5" />
              </button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="start" class="w-40">
            <DropdownMenu.Item onclick={() => onrename(query)}>Renomear</DropdownMenu.Item>
            <DropdownMenu.Separator />
            <DropdownMenu.Item variant="destructive" onclick={() => ondelete(query)}>Apagar</DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </div>
    {:else}
      <p class="px-2 py-2 text-xs leading-relaxed font-light text-muted-foreground">
        Nenhuma ainda. Use <span class="font-medium text-foreground">Salvar</span> para guardar a consulta do editor.
      </p>
    {/each}

    <p class="mt-4 px-2 pb-1 text-2xs font-medium tracking-wider text-muted-foreground uppercase">Modelos</p>
    {#each SQL_SNIPPETS as snippet (snippet.label)}
      <button class={item} onclick={() => sqlStore.open(snippet.sql)}>
        <Sparkles class="size-3.5 shrink-0" strokeWidth={1.6} />
        <span class="truncate">{snippet.label}</span>
      </button>
    {/each}
  </div>
</aside>
