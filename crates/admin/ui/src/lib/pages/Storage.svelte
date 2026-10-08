<script lang="ts">
  import * as Table from '$lib/components/ui/table'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import Plus from '@lucide/svelte/icons/plus'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte'
  import BucketDialog from '$lib/components/app/BucketDialog.svelte'
  import { api, enc } from '$lib/api'
  import { href } from '$lib/router.svelte'
  import { formatBytes } from '$lib/files'
  import type { Bucket, StorageOverview } from '$lib/types'
  import { errorMessage, intlLocale, t } from '$lib/i18n/index.svelte'

  let data = $state<StorageOverview | null>(null)
  let editing = $state<Bucket | null>(null)
  let dialogOpen = $state(false)
  let removing = $state<Bucket | null>(null)
  let confirmOpen = $state(false)

  async function load() {
    try {
      data = await api.get<StorageOverview>('/storage')
    } catch (e) {
      toast.error(errorMessage(e))
    }
  }

  $effect(() => {
    load()
  })

  const size = (bytes: number) => formatBytes(bytes, intlLocale())
  const total = $derived(data?.enabled ? data.buckets.reduce((sum, b) => sum + b.bytes, 0) : 0)

  function edit(bucket: Bucket | null) {
    editing = bucket
    dialogOpen = true
  }

  async function remove() {
    if (!removing) return
    const id = removing.id
    try {
      await api.delete(`/storage/buckets/${enc(id)}`)
      toast.success(t('storage.bucketDeleted', { bucket: id }))
      await load()
    } catch (e) {
      toast.error(errorMessage(e))
      throw e
    }
  }
</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader
    title={t('storage.title')}
    description={data?.enabled
      ? t(data.backend === 's3' ? 'storage.s3Summary' : 'storage.diskSummary', {
          count: data.buckets.length,
          size: size(total),
        })
      : undefined}
  >
    {#snippet actions()}
      {#if data?.enabled}
        <Button onclick={() => edit(null)}><Plus />{t('storage.newBucket')}</Button>
      {/if}
    {/snippet}
  </PageHeader>

  {#if data === null}
    <Skeleton class="h-48 rounded-lg" />
  {:else if !data.enabled}
    <EmptyState class="rounded-lg border" title={t('storage.disabled')} description={t('storage.disabledDescription')} />
  {:else}
    {#if data.buckets.length === 0}
      <EmptyState class="rounded-lg border" title={t('storage.empty')} description={t('storage.emptyDescription')}>
        {#snippet actions()}
          <Button variant="outline" onclick={() => edit(null)}>{t('storage.newBucket')}</Button>
        {/snippet}
      </EmptyState>
    {:else}
      <div class="overflow-hidden rounded-lg border bg-card">
        <Table.Root>
          <Table.Header>
            <Table.Row class="hover:bg-transparent">
              <Table.Head>{t('storage.columns.name')}</Table.Head>
              <Table.Head>{t('storage.columns.access')}</Table.Head>
              <Table.Head class="text-right">{t('storage.columns.files')}</Table.Head>
              <Table.Head class="text-right">{t('storage.columns.size')}</Table.Head>
              <Table.Head class="hidden md:table-cell">{t('storage.columns.limit')}</Table.Head>
              <Table.Head class="hidden md:table-cell">{t('storage.columns.types')}</Table.Head>
              <Table.Head class="w-12"><span class="sr-only">{t('common.actions')}</span></Table.Head>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {#each data.buckets as bucket (bucket.id)}
              <Table.Row>
                <Table.Cell>
                  <a class="font-mono font-medium hover:underline" href={href(`/storage/${enc(bucket.id)}`)}>{bucket.id}</a>
                </Table.Cell>
                <Table.Cell class="text-muted-foreground">
                  {bucket.public ? t('storage.public') : t('storage.private')}
                </Table.Cell>
                <Table.Cell class="text-right font-mono text-xs tabular-nums">{bucket.files}</Table.Cell>
                <Table.Cell class="text-right font-mono text-xs tabular-nums">{size(bucket.bytes)}</Table.Cell>
                <Table.Cell class="hidden text-muted-foreground md:table-cell">
                  {bucket.file_size_limit
                    ? size(bucket.file_size_limit)
                    : t('storage.serverLimit', { size: size(data.max_file_size) })}
                </Table.Cell>
                <Table.Cell class="hidden text-muted-foreground md:table-cell">
                  {#if bucket.allowed_mime_types}
                    <span class="font-mono text-xs">{bucket.allowed_mime_types.join(', ')}</span>
                  {:else}
                    {t('storage.anyType')}
                  {/if}
                </Table.Cell>
                <Table.Cell class="text-right">
                  <DropdownMenu.Root>
                    <DropdownMenu.Trigger>
                      {#snippet child({ props })}
                        <Button variant="ghost" size="icon-sm" aria-label={t('common.actions')} {...props}><Ellipsis /></Button>
                      {/snippet}
                    </DropdownMenu.Trigger>
                    <DropdownMenu.Content align="end" class="w-48">
                      <DropdownMenu.Item onclick={() => edit(bucket)}>{t('storage.edit')}</DropdownMenu.Item>
                      <DropdownMenu.Separator />
                      <DropdownMenu.Item
                        variant="destructive"
                        disabled={bucket.files > 0}
                        onclick={() => {
                          removing = bucket
                          confirmOpen = true
                        }}>{t('storage.deleteBucket')}</DropdownMenu.Item
                      >
                    </DropdownMenu.Content>
                  </DropdownMenu.Root>
                </Table.Cell>
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      </div>
    {/if}
    <p class="mt-4 text-sm text-muted-foreground">{t('storage.policiesNote')}</p>
    {#if data.backend === 'disk'}
      <p class="mt-1 text-sm text-muted-foreground">{t('storage.diskNote')}</p>
    {/if}
  {/if}
</div>

{#if data?.enabled}
  <BucketDialog bind:open={dialogOpen} bucket={editing} serverLimit={data.max_file_size} onsaved={load} />
{/if}
{#if removing}
  <ConfirmDialog
    bind:open={confirmOpen}
    title={t('storage.confirmDeleteBucket.title')}
    description={t('storage.confirmDeleteBucket.description', { bucket: removing.id })}
    confirmLabel={t('common.delete')}
    destructive
    onconfirm={remove}
  />
{/if}
