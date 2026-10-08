<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import Plus from '@lucide/svelte/icons/plus'
  import FileCode from '@lucide/svelte/icons/file-code'
  import Pencil from '@lucide/svelte/icons/pencil'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import { SQL_SNIPPETS } from '$lib/sql-snippets'
  import { sqlStore, type SavedQuery, type SqlDraft } from '$lib/sql-store.svelte'
  import { cn } from '$lib/utils'
  import { t } from '$lib/i18n/index.svelte'

  let {
    onrename,
    ondelete,
    ondeleteDraft,
  }: {
    onrename: (query: SavedQuery) => void
    ondelete: (query: SavedQuery) => void
    ondeleteDraft: (draft: SqlDraft) => void
  } = $props()

  const item =
    'group flex h-9 w-full cursor-pointer items-center gap-2 rounded-md px-2.5 text-left text-sm text-muted-foreground transition-colors hover:bg-accent/60 hover:text-foreground'
  const heading = 'px-2.5 pb-1 text-xs font-medium text-muted-foreground'
</script>

<aside class="hidden w-72 shrink-0 flex-col border-r bg-sidebar lg:flex" aria-label={t('sql.sidebar.label')}>
  <div class="flex h-14 items-center justify-between border-b pr-2 pl-4">
    <p class="text-sm font-semibold">{t('sql.sidebar.title')}</p>
    <Button variant="ghost" size="icon-sm" onclick={() => sqlStore.open('')} title={t('sql.editor.newQuery')} aria-label={t('sql.editor.newQuery')}>
      <Plus />
    </Button>
  </div>

  <div class="flex-1 overflow-y-auto px-2 py-3">
    <p class={heading}>
      {t('sql.sidebar.savedCount', { count: sqlStore.saved.length })}
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
            <FileCode class="size-4 shrink-0" aria-hidden="true" />
            <span class="truncate">{query.name}</span>
            {#if active && sqlStore.dirty}<span class="size-1.5 shrink-0 rounded-full bg-muted-foreground" title={t('sql.editor.unsavedTitle')}></span>{/if}
          </button>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger>
              {#snippet child({ props })}
                <button
                  {...props}
                  class="grid size-7 shrink-0 cursor-pointer place-items-center rounded-md opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover:bg-accent aria-expanded:opacity-100"
                  aria-label={t('sql.sidebar.actionsFor', { name: query.name })}
                >
                  <Ellipsis class="size-4" />
                </button>
              {/snippet}
            </DropdownMenu.Trigger>
            <DropdownMenu.Content align="start" class="w-44">
              <DropdownMenu.Item onclick={() => onrename(query)}><Pencil aria-hidden="true" />{t('sql.sidebar.rename')}</DropdownMenu.Item>
              <DropdownMenu.Separator />
              <DropdownMenu.Item variant="destructive" onclick={() => ondelete(query)}><Trash2 aria-hidden="true" />{t('common.delete')}</DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </div>
      {:else}
        <p class="px-2.5 py-1 text-xs text-muted-foreground">{t('sql.sidebar.noSaved')}</p>
      {/each}
    </div>

    {#if sqlStore.looseDrafts.length}
      <p class={cn(heading, 'mt-6')}>{t('sql.editor.drafts')}</p>
      <div class="grid gap-0.5">
        {#each sqlStore.looseDrafts as draft (draft.id)}
          <div class={cn(item, 'pr-1', sqlStore.activeDraftId === draft.id && 'bg-sidebar-accent text-foreground')}>
            <button class="flex min-w-0 flex-1 items-center gap-2" onclick={() => sqlStore.openDraft(draft.id)} aria-current={sqlStore.activeDraftId === draft.id ? 'true' : undefined}><FileCode class="size-4 shrink-0" aria-hidden="true" /><span class="truncate">{draft.sql.trim().split('\n')[0]}</span></button>
            <Button variant="ghost" size="icon-xs" aria-label={t('common.discard')} onclick={() => ondeleteDraft(draft)}><Trash2 aria-hidden="true" /></Button>
          </div>
        {/each}
      </div>
    {/if}
    <p class="px-2.5 pt-3 text-xs text-muted-foreground">{t('sql.editor.localOnly')}</p>

    <p class={cn(heading, 'mt-6')}>{t('sql.editor.templates')}</p>
    <div class="grid gap-0.5">
      {#each SQL_SNIPPETS as snippet (snippet.label)}
        <button class={item} onclick={() => sqlStore.open(t(snippet.sql))}>
          <FileCode class="size-4 shrink-0" aria-hidden="true" />
          <span class="min-w-0 flex-1 truncate">{t(snippet.label)}</span>
        </button>
      {/each}
    </div>
  </div>
</aside>
