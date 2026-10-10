<script lang="ts">
  import { untrack, onDestroy } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import Folder from '@lucide/svelte/icons/folder'
  import Upload from '@lucide/svelte/icons/upload'
  import Download from '@lucide/svelte/icons/download'
  import Copy from '@lucide/svelte/icons/copy'
  import Link from '@lucide/svelte/icons/link'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import FileText from '@lucide/svelte/icons/file-text'
  import Image from '@lucide/svelte/icons/image'
  import FileIcon from '@lucide/svelte/icons/file'
  import Eye from '@lucide/svelte/icons/eye'
  import CircleCheck from '@lucide/svelte/icons/circle-check'
  import CircleAlert from '@lucide/svelte/icons/circle-alert'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import ChevronRight from '@lucide/svelte/icons/chevron-right'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/shared/PageHeader.svelte'
  import BucketAccess from '$lib/features/storage/components/BucketAccess.svelte'
  import EmptyState from '$lib/components/shared/EmptyState.svelte'
  import ConfirmDialog from '$lib/components/shared/ConfirmDialog.svelte'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import FilePreviewDialog from '$lib/features/storage/components/FilePreviewDialog.svelte'
  import { StorageBrowser } from '$lib/features/storage/browser.svelte'
  import { storageBrowserAdapter } from '$lib/features/storage/api'
  import { type UploadTarget } from '$lib/features/storage/upload-queue.svelte'
  import { copyText } from '$lib/clipboard'
  import { enc } from '$lib/api'
  import { href, route } from '$lib/router.svelte'
  import { baseName, folderTrail, formatBytes, publicUrl } from '$lib/features/storage/files'
  import type { StoredFile } from '$lib/types'
  import { errorMessage, intlLocale, t } from '$lib/i18n/index.svelte'

  let { bucket }: { bucket: string } = $props()

  const browser = new StorageBrowser(storageBrowserAdapter)
  const infoResource = browser.infoResource
  const filesResource = browser.filesResource
  const uploads = browser.uploads
  const info = $derived(infoResource.loaded && !infoResource.error ? infoResource.data : undefined)
  const files = $derived(filesResource.data?.objects ?? null)
  const folders = $derived(filesResource.data?.folders ?? [])
  const hasNext = $derived(filesResource.data?.has_next ?? false)
  onDestroy(() => browser.cancel())
  const uploading = $derived(uploads.progress)
  const queue = $derived(uploads.items)
  let conflicts = $state<File[]>([])
  let conflictTarget = $state<{ bucket: string; prefix: string } | null>(null)
  let replaceOpen = $state(false)
  let removing = $state<StoredFile | null>(null)
  let removeOpen = $state(false)
  let picker = $state<HTMLInputElement>()
  const loading = $derived(filesResource.loading)
  const error = $derived(filesResource.error ? errorMessage(filesResource.error) : '')
  const infoError = $derived(infoResource.error ? errorMessage(infoResource.error) : '')
  const infoLoading = $derived(infoResource.loading)
  let preview = $state<StoredFile | null>(null)
  let previewOpen = $state(false)
  let dragging = $state(false)
  let dragDepth = 0
  const prefix = $derived(route.query.get('prefix') ?? '')
  const trail = $derived(folderTrail(prefix))
  const base = $derived(`/storage/buckets/${enc(bucket)}`)
  const folderHref = (p: string) => href(`/storage/${enc(bucket)}${p ? `?prefix=${enc(p)}` : ''}`)
  const fileHref = (name: string) => `/admin/api${base}/file?${new URLSearchParams({ name })}`
  const fileIcon = (file: StoredFile) => file.mime_type.startsWith('image/') ? Image : file.mime_type.startsWith('text/') || ['application/pdf', 'application/json'].includes(file.mime_type) ? FileText : FileIcon
  function showPreview(file: StoredFile) { preview = file; previewOpen = true }

  const loadInfo = async () => { await browser.loadInfo() }
  const load = async (more = false) => { await browser.load(more) }
  $effect(() => {
    const target = { bucket, prefix }
    untrack(() => { void browser.open(target) })
  })

  async function send(list: File[], replace: boolean, target?: UploadTarget) {
    if (!info && !target) return
    const result = await browser.send(list, replace, target)
    if (!result) return
    for (const { file, error } of result.errors) toast.error(`${file.name}: ${errorMessage(error)}`)
    if (result.sent) toast.success(t('storage.browser.uploaded', { count: result.sent }))
    if (result.conflicts.length) {
      conflicts = result.conflicts
      conflictTarget = result.target
      replaceOpen = true
    }
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
      await browser.remove(name)
      toast.success(t('storage.browser.deleted', { name: baseName(name) }))
    } catch (e) {
      toast.error(errorMessage(e))
      throw e
    }
  }

  function dropped(event: DragEvent) {
    event.preventDefault()
    dragging = false
    dragDepth = 0
    if (event.dataTransfer?.files.length && !uploading && info) send([...event.dataTransfer.files], false)
  }

  const date = $derived(new Intl.DateTimeFormat(intlLocale(), { dateStyle: 'short', timeStyle: 'short' }))
  const size = (bytes: number) => formatBytes(bytes, intlLocale())
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="mx-auto w-full max-w-page px-4 pt-2 pb-12 sm:px-6 lg:px-8" ondragenter={(event) => { if (event.dataTransfer?.types.includes('Files')) { event.preventDefault(); dragDepth++; dragging = true } }} ondragleave={() => { dragDepth = Math.max(0, dragDepth - 1); if (!dragDepth) dragging = false }} ondragover={(event) => { if (event.dataTransfer?.types.includes('Files')) event.preventDefault() }} ondrop={dropped}>
  <nav class="mb-4 flex flex-wrap items-center gap-2 text-sm [overflow-wrap:anywhere]" aria-label={t('storage.browser.path')}>
    <a class="text-muted-foreground hover:text-foreground hover:underline" href={href('/storage')}>{t('storage.title')}</a>
    <ChevronRight class="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
    {#if prefix}<a class="font-mono text-muted-foreground hover:text-foreground hover:underline" href={folderHref('')}>{bucket}</a>
    {:else}<span class="font-mono" aria-current="page">{bucket}</span>{/if}
    {#each trail as folder, index (folder.prefix)}
      <ChevronRight class="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
      {#if index === trail.length - 1}<span class="font-mono" aria-current="page">{folder.name}</span>
      {:else}<a class="font-mono text-muted-foreground hover:text-foreground hover:underline" href={folderHref(folder.prefix)}>{folder.name}</a>{/if}
    {/each}
  </nav>
  <PageHeader
    title={bucket}
    description={info ? `${info.bucket.public ? t('storage.public') : t('storage.private')} · ${size(info.bucket.bytes)}` : undefined}
  >
    {#snippet actions()}
      {#if info}
        <input bind:this={picker} type="file" multiple accept={info.bucket.allowed_mime_types?.join(',')} class="hidden" onchange={picked} />
        <Button disabled={uploading !== null} onclick={() => picker?.click()}>
          {#if uploading}<LoaderCircle data-icon="inline-start" class="animate-spin" aria-hidden="true" />{:else}<Upload data-icon="inline-start" aria-hidden="true" />{/if}
          {uploading ? t('storage.browser.uploading', uploading) : t('storage.browser.upload')}
        </Button>
        <Button variant="outline" href="#bucket-access" aria-label={t('storage.browser.access')}><ShieldCheck data-icon="inline-start" aria-hidden="true" /><span class="sm:hidden">{t('storage.browser.accessShort')}</span><span class="hidden sm:inline">{t('storage.browser.access')}</span></Button>
      {/if}
    {/snippet}
  </PageHeader>

  {#if infoError}<LoadError message={infoError} onretry={loadInfo} busy={infoLoading} />{/if}
  {#if error}<LoadError message={error} onretry={() => load()} busy={loading} />{/if}
  <div class="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]">
  <section class="min-w-0" aria-label={t('storage.browser.files')}>
  {#if info}
    <button type="button" disabled={uploading !== null} class={['mb-4 hidden w-full cursor-pointer flex-col items-center gap-2 rounded-xl border-2 border-dashed border-border bg-card px-4 py-4 text-sm text-muted-foreground transition-colors hover:border-brand/50 hover:text-foreground disabled:cursor-wait sm:flex', dragging && 'border-brand bg-brand/5 text-brand']} onclick={() => picker?.click()}><Upload class="size-5" aria-hidden="true" />{t('storage.browser.dropHint')}</button>
  {/if}
  {#if queue.length}
    <section class="mb-4 grid gap-3 rounded-xl border bg-card p-4" aria-label={t('storage.browser.uploadProgress')}>
      <div class="flex items-center justify-between"><h2 class="text-sm font-medium">{t('storage.browser.uploadProgress')}</h2>{#if !uploading}<Button variant="ghost" size="sm" onclick={() => uploads.clear()}>{t('common.close')}</Button>{/if}</div>
      {#each queue as item, index (index)}
        <div class="grid gap-1"><div class="flex min-w-0 items-center gap-2 text-xs">{#if item.status === 'done'}<CircleCheck class="size-4 shrink-0 text-brand" aria-hidden="true" />{:else if item.status === 'error' || item.status === 'conflict'}<CircleAlert class="size-4 shrink-0 text-warning" aria-hidden="true" />{:else}<Upload class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />{/if}<span class="min-w-0 flex-1 truncate">{item.name}</span><span class="shrink-0 text-muted-foreground" aria-live="polite">{t(`storage.browser.uploadStatus.${item.status}`)}</span></div>{#if item.status === 'sending' || item.status === 'queued'}<progress class="h-1.5 w-full accent-brand" value={item.loaded} max={item.total || 1} aria-label={item.name}></progress>{/if}</div>
      {/each}
    </section>
  {/if}

  {#if info === null}
    <EmptyState class="rounded-xl border bg-card" title={t('storage.browser.notFound', { bucket })}>
      {#snippet actions()}
        <Button variant="outline" href={href('/storage')}>{t('storage.browser.back')}</Button>
      {/snippet}
    </EmptyState>
  {:else}
    {#if files === null}
      {#if loading}<Skeleton class="h-48 rounded-xl" />{/if}
    {:else if files.length === 0 && folders.length === 0}
      <EmptyState
        class="rounded-xl border bg-card"
        icon={Folder}
        title={prefix ? t('storage.browser.emptyFolder') : t('storage.browser.emptyBucket')}
        description={t('storage.browser.emptyFolderDescription')}
      >{#snippet actions()}<Button variant="outline" disabled={!info || uploading !== null} onclick={() => picker?.click()}><Upload data-icon="inline-start" aria-hidden="true" />{t('storage.browser.upload')}</Button>{/snippet}</EmptyState>
    {:else}
      <div class="overflow-hidden rounded-xl border bg-card">
        <Table.Root class="table-fixed md:table-auto">
          <Table.Header>
            <Table.Row class="hover:bg-transparent">
              <Table.Head>{t('storage.browser.columns.name')}</Table.Head>
              <Table.Head class="w-20 text-right 2xl:w-auto">{t('storage.browser.columns.size')}</Table.Head>
              <Table.Head class="hidden 2xl:table-cell">{t('storage.browser.columns.type')}</Table.Head>
              <Table.Head class="hidden md:table-cell xl:hidden 2xl:table-cell">{t('storage.browser.columns.updated')}</Table.Head>
              <Table.Head class="w-24"><span class="sr-only">{t('common.actions')}</span></Table.Head>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {#each folders as folder (folder)}
              <Table.Row>
                <Table.Cell class="max-w-0">
                  <a class="flex min-w-0 items-center gap-2 font-medium hover:underline" href={folderHref(`${prefix}${folder}/`)}>
                    <Folder class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" /><span class="truncate">{folder}/</span>
                  </a>
                </Table.Cell>
                <Table.Cell></Table.Cell><Table.Cell class="hidden 2xl:table-cell"></Table.Cell><Table.Cell class="hidden md:table-cell xl:hidden 2xl:table-cell"></Table.Cell><Table.Cell></Table.Cell>
              </Table.Row>
            {/each}
            {#each files as file (file.id)}
              {@const Icon = fileIcon(file)}
              <Table.Row>
                <Table.Cell class="max-w-0 font-medium"><button type="button" class="flex w-full min-w-0 cursor-pointer items-center gap-2 text-left hover:text-brand" onclick={() => showPreview(file)} title={file.name}><Icon class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" /><span class="truncate">{baseName(file.name)}</span></button></Table.Cell>
                <Table.Cell class="text-right font-mono text-xs tabular-nums">{size(file.size)}</Table.Cell>
                <Table.Cell class="hidden font-mono text-xs text-muted-foreground 2xl:table-cell">{file.mime_type}</Table.Cell>
                <Table.Cell class="hidden text-muted-foreground md:table-cell xl:hidden 2xl:table-cell">{date.format(new Date(file.updated_at))}</Table.Cell>
                <Table.Cell class="text-right">
                  <div class="flex items-center justify-end gap-1">
                  <Button variant="ghost" size="icon-sm" href={fileHref(file.name)} download title={t('storage.browser.downloadFile', { name: baseName(file.name) })} aria-label={t('storage.browser.downloadFile', { name: baseName(file.name) })}><Download aria-hidden="true" /></Button>
                  <DropdownMenu.Root>
                    <DropdownMenu.Trigger>
                      {#snippet child({ props })}
                        <Button variant="ghost" size="icon-sm" aria-label={t('common.actionsFor', { name: baseName(file.name) })} {...props}><Ellipsis aria-hidden="true" /></Button>
                      {/snippet}
                    </DropdownMenu.Trigger>
                    <DropdownMenu.Content align="end" class="w-52">
                      <DropdownMenu.Group>
                      <DropdownMenu.Item onclick={() => showPreview(file)}><Eye aria-hidden="true" />{t('storage.browser.preview')}</DropdownMenu.Item>
                      <DropdownMenu.Item>
                        {#snippet child({ props })}
                          <a {...props} href={`/admin/api${base}/file?${new URLSearchParams({ name: file.name })}`} download
                            ><Download aria-hidden="true" />{t('storage.browser.download')}</a
                          >
                        {/snippet}
                      </DropdownMenu.Item>
                      {#if info?.bucket.public}
                        <DropdownMenu.Item
                          onclick={() => copyText(publicUrl(info!.publicOrigin, bucket, file.name), t('storage.browser.urlCopied'))}
                          ><Link aria-hidden="true" />{t('storage.browser.copyUrl')}</DropdownMenu.Item
                        >
                      {/if}
                      <DropdownMenu.Item onclick={() => copyText(file.name, t('storage.browser.pathCopied'))}>
                        <Copy aria-hidden="true" />
                        {t('storage.browser.copyPath')}
                      </DropdownMenu.Item>
                      </DropdownMenu.Group>
                      <DropdownMenu.Separator />
                      <DropdownMenu.Group>
                      <DropdownMenu.Item
                        variant="destructive"
                        onclick={() => {
                          removing = file
                          removeOpen = true
                        }}><Trash2 aria-hidden="true" />{t('storage.browser.delete')}</DropdownMenu.Item
                      >
                      </DropdownMenu.Group>
                    </DropdownMenu.Content>
                  </DropdownMenu.Root>
                  </div>
                </Table.Cell>
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      </div>
      {#if hasNext}
        <div class="mt-4 flex justify-center">
          <Button variant="outline" size="sm" disabled={loading} onclick={() => load(true)}>{t('storage.browser.more')}</Button>
        </div>
      {/if}
    {/if}
  {/if}
  </section>
  {#if info}<aside class="min-w-0 xl:sticky xl:top-4"><BucketAccess {bucket} /></aside>{/if}
  </div>
</div>

{#if preview}<FilePreviewDialog bind:open={previewOpen} file={preview} url={fileHref(preview.name)} />{/if}

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
    destination: `${conflictTarget?.bucket ?? bucket}/${conflictTarget?.prefix ?? prefix}`,
  })}
  confirmLabel={t('storage.browser.replace.confirm')}
  onconfirm={() => send(conflicts, true, conflictTarget ?? undefined)}
/>
