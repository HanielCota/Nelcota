<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import FileCode from '@lucide/svelte/icons/file-code'
  import Sparkles from '@lucide/svelte/icons/sparkles'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import FilePlus from '@lucide/svelte/icons/file-plus'
  import Pencil from '@lucide/svelte/icons/pencil'
  import Trash2 from '@lucide/svelte/icons/trash-2'
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
    'group relative flex h-9 w-full cursor-pointer items-center gap-2.5 rounded-lg px-2.5 text-left text-sm font-medium text-muted-foreground transition-colors hover:bg-accent/60 hover:text-foreground'
  const heading = 'px-2.5 pb-1.5 text-3xs font-semibold tracking-[0.08em] text-muted-foreground/80 uppercase'
</script>

<aside class="hidden w-72 shrink-0 flex-col border-r bg-sidebar lg:flex" aria-label="Consultas salvas e modelos">
  <div class="flex h-14 items-center justify-between border-b px-4">
    <p class="text-sm font-semibold">Editor SQL</p>
    <Button variant="outline" size="sm" onclick={() => sqlStore.open('')} title="Nova consulta">
      <FilePlus />Nova
    </Button>
  </div>

  <div class="flex-1 overflow-y-auto px-2 py-3">
    <p class={heading}>
      Salvas <span class="tabular-nums">({sqlStore.saved.length})</span>
    </p>
    <div class="grid gap-0.5">
      {#each sqlStore.sorted as query (query.id)}
        {@const active = query.id === sqlStore.currentId}
        <div class={cn(item, 'pr-1', active && 'bg-sidebar-accent text-foreground shadow-card')}>
          {#if active}
            <span class="absolute inset-y-2 left-0 w-[3px] rounded-r-full bg-brand" aria-hidden="true"></span>
          {/if}
          <button
            class="flex h-full min-w-0 flex-1 cursor-pointer items-center gap-2.5"
            aria-current={active ? 'true' : undefined}
            onclick={() => sqlStore.openSaved(query.id)}
          >
            <FileCode class={['size-4 shrink-0', active && 'text-brand']} strokeWidth={1.75} />
            <span class="truncate">{query.name}</span>
            {#if active && sqlStore.dirty}<span class="size-2 shrink-0 rounded-full bg-warning" title="Alterações não salvas"></span>{/if}
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
              <DropdownMenu.Item onclick={() => onrename(query)}><Pencil />Renomear</DropdownMenu.Item>
              <DropdownMenu.Separator />
              <DropdownMenu.Item variant="destructive" onclick={() => ondelete(query)}><Trash2 />Apagar</DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </div>
      {:else}
        <p class="mx-1 rounded-lg border border-dashed px-3 py-3 text-xs leading-relaxed text-muted-foreground">
          Nenhuma ainda. Use <span class="font-semibold text-foreground">Salvar</span> para guardar a consulta do editor.
        </p>
      {/each}
    </div>

    <p class={cn(heading, 'mt-6')}>Modelos</p>
    <div class="grid gap-0.5">
      {#each SQL_SNIPPETS as snippet (snippet.label)}
        <button class={item} onclick={() => sqlStore.open(snippet.sql)}>
          <Sparkles class="size-4 shrink-0" strokeWidth={1.75} />
          <span class="truncate">{snippet.label}</span>
        </button>
      {/each}
    </div>
  </div>
</aside>
