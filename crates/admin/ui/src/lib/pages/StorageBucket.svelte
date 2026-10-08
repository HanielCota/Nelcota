<script lang="ts">
  import * as Table from '$lib/components/ui/table'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import Folder from '@lucide/svelte/icons/folder'
  import Upload from '@lucide/svelte/icons/upload'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte'
  import { ApiError, api, enc, uploadFile } from '$lib/api'
  import { href, route } from '$lib/router.svelte'
  import { baseName, folderTrail, formatBytes, publicUrl } from '$lib/files'
  import type { Bucket, StorageListing, StorageOverview, StoredFile } from '$lib/types'
  import { errorMessage, intlLocale, t } from '$lib/i18n/index.svelte'

  let { bucket }: { bucket: string } = $props()

  let info = $state<{ bucket: Bucket; publicOrigin: string } | null | undefined>(undefined)
  let folders = $state<string[]>([])
  let files = $state<StoredFile[] | null>(null)
  let hasNext = $state(false)
  let uploading = $state<{ done: number; total: number } | null>(null)
  let conflicts = $state<File[]>([])
  let replaceOpen = $state(false)
  let removing = $state<StoredFile | null>(null)
  let removeOpen = $state(false)
  let picker = $state<HTMLInputElement>()

  const prefix = $derived(route.query.get('prefix') ?? '')
  const base = $derived(`/storage/buckets/${enc(bucket)}`)
  const folderHref = (p: string) => href(`/storage/${enc(bucket)}${p ? `?prefix=${enc(p)}` : ''}`)

  async function loadInfo() {
    try {
      const data = await api.get<StorageOverview>('/storage')
      const found = data.enabled ? data.buckets.find((b) => b.id === bucket) : undefined
      info =
        data.enabled && found ? { bucket: found, publicOrigin: data.public_url ?? location.origin } : null
    } catch (e) {
      toast.error(errorMessage(e))
    }
  }

  async function load(more = false) {
    try {
      const params = new URLSearchParams({ prefix, offset: String(more ? (files?.length ?? 0) : 0) })
      const page = await api.get<StorageListing>(`${base}/objects?${params}`)
      folders = page.folders
      files = more ? [...(files ?? []), ...page.objects] : page.objects
      hasNext = page.has_next
    } catch (e) {
      toast.error(errorMessage(e))
    }
  }

  $effect(() => {
    loadInfo()
  })

  $effect(() => {
    void prefix
    files = null
    load()
  })

  async function send(list: File[], replace: boolean) {
    const clashes: File[] = []
    let sent = 0
    uploading = { done: 0, total: list.length }
    for (const file of list) {
      const params = new URLSearchParams({ name: prefix + file.name })
      if (replace) params.set('replace', 'true')
      try {
        await uploadFile(`${base}/upload?${params}`, file)
        sent++
      } catch (e) {
        if (e instanceof ApiError && e.code === 'object_exists') clashes.push(file)
        else toast.error(`${file.name}: ${errorMessage(e)}`)
      }
      uploading = { done: uploading.done + 1, total: list.length }
    }
    uploading = null
    if (sent) toast.success(t('storage.browser.uploaded', { count: sent }))
    if (clashes.length) {
      conflicts = clashes
      replaceOpen = true
    }
    await Promise.all([load(), loadInfo()])
  }

  function picked() {
    if (!picker) return
    const list = [...(picker.files ?? [])]
    picker.value = ''
    if (list.length) send(list, false)
  }

  async function remove() {
    if (!removing) return
    const name = removing.name
    try {
      await api.delete(`${base}/file?${new URLSearchParams({ name })}`)
      toast.success(t('storage.browser.deleted', { name: baseName(name) }))
      await Promise.all([load(), loadInfo()])
    } catch (e) {
      toast.error(errorMessage(e))
      throw e
    }
  }

  function copy(text: string, message: string) {
    navigator.clipboard.writeText(text)
    toast.success(message)
  }

  const date = $derived(new Intl.DateTimeFormat(intlLocale(), { dateStyle: 'short', timeStyle: 'short' }))
  const size = (bytes: number) => formatBytes(bytes, intlLocale())
</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader
    title={bucket}
    description={info ? `${info.bucket.public ? t('storage.public') : t('storage.private')} · ${size(info.bucket.bytes)}` : undefined}
  >
    {#snippet actions()}
      {#if info}
        <input bind:this={picker} type="file" multiple class="hidden" onchange={picked} />
        <Button disabled={uploading !== null} onclick={() => picker?.click()}>
          <Upload />
          {uploading ? t('storage.browser.uploading', uploading) : t('storage.browser.upload')}
        </Button>
      {/if}
    {/snippet}
  </PageHeader>

  {#if info === null}
    <EmptyState class="rounded-lg border" title={t('storage.browser.notFound', { bucket })}>
      {#snippet actions()}
        <Button variant="outline" href={href('/storage')}>{t('storage.browser.back')}</Button>
      {/snippet}
    </EmptyState>
  {:else}
    <nav class="mb-3 flex flex-wrap items-center gap-1 font-mono text-sm" aria-label={t('storage.browser.path')}>
      <a class="text-muted-foreground hover:text-foreground hover:underline" href={folderHref('')}>{bucket}</a>
      {#each folderTrail(prefix) as folder (folder.prefix)}
        <span class="text-muted-foreground">/</span>
        <a class="text-muted-foreground hover:text-foreground hover:underline" href={folderHref(folder.prefix)}>{folder.name}</a>
      {/each}
      <span class="text-muted-foreground">/</span>
    </nav>

    {#if files === null}
      <Skeleton class="h-48 rounded-lg" />
    {:else if files.length === 0 && folders.length === 0}
      <EmptyState
        class="rounded-lg border"
        title={prefix ? t('storage.browser.emptyFolder') : t('storage.browser.emptyBucket')}
        description={t('storage.browser.emptyFolderDescription')}
      />
    {:else}
      <div class="overflow-hidden rounded-lg border bg-card">
        <Table.Root>
          <Table.Header>
            <Table.Row class="hover:bg-transparent">
              <Table.Head>{t('storage.browser.columns.name')}</Table.Head>
              <Table.Head class="text-right">{t('storage.browser.columns.size')}</Table.Head>
              <Table.Head class="hidden md:table-cell">{t('storage.browser.columns.type')}</Table.Head>
              <Table.Head class="hidden md:table-cell">{t('storage.browser.columns.updated')}</Table.Head>
              <Table.Head class="w-12"><span class="sr-only">{t('common.actions')}</span></Table.Head>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {#each folders as folder (folder)}
              <Table.Row>
                <Table.Cell colspan={5}>
                  <a class="inline-flex items-center gap-2 font-medium hover:underline" href={folderHref(`${prefix}${folder}/`)}>
                    <Folder class="size-4 text-muted-foreground" aria-hidden="true" />{folder}/
                  </a>
                </Table.Cell>
              </Table.Row>
            {/each}
            {#each files as file (file.id)}
              <Table.Row>
                <Table.Cell class="max-w-0 truncate font-medium" title={file.name}>{baseName(file.name)}</Table.Cell>
                <Table.Cell class="text-right font-mono text-xs tabular-nums">{size(file.size)}</Table.Cell>
                <Table.Cell class="hidden font-mono text-xs text-muted-foreground md:table-cell">{file.mime_type}</Table.Cell>
                <Table.Cell class="hidden text-muted-foreground md:table-cell">{date.format(new Date(file.updated_at))}</Table.Cell>
                <Table.Cell class="text-right">
                  <DropdownMenu.Root>
                    <DropdownMenu.Trigger>
                      {#snippet child({ props })}
                        <Button variant="ghost" size="icon-sm" aria-label={t('common.actions')} {...props}><Ellipsis /></Button>
                      {/snippet}
                    </DropdownMenu.Trigger>
                    <DropdownMenu.Content align="end" class="w-52">
                      <DropdownMenu.Item>
                        {#snippet child({ props })}
                          <a {...props} href={`/admin/api${base}/file?${new URLSearchParams({ name: file.name })}`} download
                            >{t('storage.browser.download')}</a
                          >
                        {/snippet}
                      </DropdownMenu.Item>
                      {#if info?.bucket.public}
                        <DropdownMenu.Item
                          onclick={() => copy(publicUrl(info!.publicOrigin, bucket, file.name), t('storage.browser.urlCopied'))}
                          >{t('storage.browser.copyUrl')}</DropdownMenu.Item
                        >
                      {/if}
                      <DropdownMenu.Item onclick={() => copy(file.name, t('storage.browser.pathCopied'))}>
                        {t('storage.browser.copyPath')}
                      </DropdownMenu.Item>
                      <DropdownMenu.Separator />
                      <DropdownMenu.Item
                        variant="destructive"
                        onclick={() => {
                          removing = file
                          removeOpen = true
                        }}>{t('storage.browser.delete')}</DropdownMenu.Item
                      >
                    </DropdownMenu.Content>
                  </DropdownMenu.Root>
                </Table.Cell>
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      </div>
      {#if hasNext}
        <div class="mt-4 flex justify-center">
          <Button variant="outline" size="sm" onclick={() => load(true)}>{t('storage.browser.more')}</Button>
        </div>
      {/if}
    {/if}
  {/if}
</div>

{#if removing}
  <ConfirmDialog
    bind:open={removeOpen}
    title={t('storage.browser.confirmDelete.title')}
    description={t('storage.browser.confirmDelete.description', { name: baseName(removing.name) })}
    confirmLabel={t('common.delete')}
    destructive
    onconfirm={remove}
  />
{/if}
<ConfirmDialog
  bind:open={replaceOpen}
  title={t('storage.browser.replace.title')}
  description={t('storage.browser.replace.description', {
    count: conflicts.length,
    names: conflicts.map((f) => f.name).join(', '),
  })}
  confirmLabel={t('storage.browser.replace.confirm')}
  onconfirm={() => send(conflicts, true)}
/>
