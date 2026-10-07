<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import Pencil from '@lucide/svelte/icons/pencil'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import Plus from '@lucide/svelte/icons/plus'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import type { ColumnInfo, Structure } from '$lib/ddl'
  import { t } from '$lib/i18n/index.svelte'

  let {
    structure,
    onadd,
    onedit,
    ondelete,
  }: {
    structure: Structure
    onadd: () => void
    onedit: (column: ColumnInfo) => void
    ondelete: (column: ColumnInfo) => void
  } = $props()

  const tag = 'rounded-md border border-border-strong bg-muted px-1.5 py-0.5 text-3xs font-medium text-muted-foreground'
</script>

<section class="overflow-hidden rounded-lg border bg-card">
  <header class="flex items-center justify-between gap-4 border-b px-5 py-4">
    <div>
      <h2 class="text-base font-semibold">{t('tables.structure.columnsTitle')}</h2>
      <p class="text-sm text-muted-foreground">{t('tables.structure.columnsCount', { count: structure.columns.length })}</p>
    </div>
    <Button onclick={onadd}><Plus />{t('tables.structure.newColumn')}</Button>
  </header>
  <div class="divide-y">
    {#each structure.columns as column (column.name)}
      <div class="group grid items-center gap-x-4 gap-y-1 px-5 py-3 text-sm transition-colors hover:bg-muted/40 md:grid-cols-[minmax(10rem,14rem)_minmax(8rem,12rem)_1fr_auto]">
        <div class="flex min-w-0 items-center gap-2">
          {#if column.primary_key}<KeyRound class="size-3.5 shrink-0 text-muted-foreground" aria-label={t('tables.structure.primaryKey')} />{/if}
          <span class="truncate font-mono text-sm font-medium">{column.name}</span>
        </div>
        <span class="truncate font-mono text-xs text-muted-foreground">{column.data_type}</span>
        <div class="flex min-w-0 flex-wrap items-center gap-1.5">
          {#if column.identity}<span class={tag}>identity</span>{/if}
          {#if column.generated}<span class={tag}>{t('tables.structure.generated')}</span>{/if}
          {#if !column.nullable}<span class={tag}>{t('tables.structure.required')}</span>{/if}
          {#if column.unique}<span class={tag}>{t('tables.structure.unique')}</span>{/if}
          {#if column.references}
            <span class={tag}>→ {column.references.table}.{column.references.column}</span>
          {/if}
          {#if column.default && !column.identity && !column.generated}
            <span class="truncate font-mono text-2xs text-muted-foreground" title={column.default}>= {column.default}</span>
          {/if}
          {#if column.comment}<span class="truncate text-xs text-muted-foreground">{column.comment}</span>{/if}
        </div>
        <div class="flex justify-end gap-1 md:opacity-0 md:group-hover:opacity-100 md:focus-within:opacity-100">
          <Button variant="ghost" size="icon-sm" aria-label={t('tables.structure.edit', { name: column.name })} onclick={() => onedit(column)}>
            <Pencil />
          </Button>
          <Button variant="ghost" size="icon-sm" aria-label={t('tables.structure.delete', { name: column.name })} onclick={() => ondelete(column)}>
            <Trash2 />
          </Button>
        </div>
      </div>
    {/each}
  </div>
</section>
