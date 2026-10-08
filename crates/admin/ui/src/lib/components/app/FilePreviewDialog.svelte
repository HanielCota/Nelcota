<script lang="ts">
  import { untrack } from 'svelte'
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Download from '@lucide/svelte/icons/download'
  import FileText from '@lucide/svelte/icons/file-text'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import LoadError from './LoadError.svelte'
  import CodeBlock from './CodeBlock.svelte'
  import { ApiError, isAbort } from '$lib/api'
  import { baseName, formatBytes, previewKind } from '$lib/files'
  import type { StoredFile } from '$lib/types'
  import { errorMessage, intlLocale, t } from '$lib/i18n/index.svelte'

  let { open = $bindable(false), file, url }: { open?: boolean; file: StoredFile; url: string } = $props()
  let loading = $state(false)
  let error = $state('')
  let media = $state('')
  let text = $state('')
  let unavailable = $state(false)
  let controller: AbortController | undefined
  let objectUrl = ''
  const kind = $derived(previewKind(file.mime_type, file.size))

  function release() {
    controller?.abort()
    if (objectUrl) URL.revokeObjectURL(objectUrl)
    objectUrl = ''
  }

  async function load() {
    release()
    const request = (controller = new AbortController())
    const target = file
    const format = kind
    media = ''; text = ''; error = ''; unavailable = !format
    if (!format) { loading = false; return }
    loading = true
    try {
      const response = await fetch(url, { credentials: 'same-origin', signal: request.signal })
      if (!response.ok) throw new ApiError(t('common.requestFailed'), response.status)
      const blob = await response.blob()
      if (request.signal.aborted) return
      if (blob.size > (format === 'text' ? 1 : 20) * 1024 * 1024) { unavailable = true; return }
      if (format === 'text') {
        const content = await blob.text()
        if (!request.signal.aborted) text = content
      }
      else if (format === 'pdf') {
        objectUrl = URL.createObjectURL(new Blob([blob], { type: 'application/pdf' }))
        media = objectUrl
      } else {
        const reader = new FileReader()
        const result = await new Promise<string>((resolve, reject) => {
          reader.onload = () => resolve(String(reader.result))
          reader.onerror = () => reject(reader.error)
          reader.readAsDataURL(new Blob([blob], { type: target.mime_type }))
        })
        if (!request.signal.aborted) media = result
      }
    } catch (e) {
      if (!isAbort(e)) error = errorMessage(e)
    } finally {
      if (controller === request) loading = false
    }
  }

  $effect(() => {
    void [file.id, url]
    if (open) untrack(load)
    return release
  })
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-3xl">
    <Dialog.Header>
      <Dialog.Title class="break-all pr-6">{baseName(file.name)}</Dialog.Title>
      <Dialog.Description class="break-all">{file.name} · {file.mime_type} · {formatBytes(file.size, intlLocale())}</Dialog.Description>
    </Dialog.Header>
    {#if error}<LoadError message={error} onretry={load} busy={loading} />
    {:else if loading}<Skeleton class="h-48" />
    {:else if unavailable}<p class="text-sm text-muted-foreground">{t('storage.browser.previewUnavailable')}</p>
    {:else if kind === 'image' && media}<img src={media} alt={baseName(file.name)} class="mx-auto max-h-[60dvh] max-w-full rounded-md object-contain" onerror={() => (error = t('common.requestFailed'))} />
    {:else if kind === 'text'}<CodeBlock code={text} wrap />
    {:else if kind === 'pdf' && media}<div class="grid justify-items-center gap-3 rounded-lg border bg-muted/30 p-8"><FileText class="size-12 text-muted-foreground" aria-hidden="true" /><p class="text-sm text-muted-foreground">{t('storage.browser.pdfHint')}</p><Button variant="outline" href={media} target="_blank" rel="noopener">{t('storage.browser.openPdf')}<ArrowUpRight data-icon="inline-end" aria-hidden="true" /></Button></div>{/if}
    <Dialog.Footer><Button variant="outline" onclick={() => (open = false)}>{t('common.close')}</Button><Button href={url} download><Download data-icon="inline-start" aria-hidden="true" />{t('common.download')}</Button></Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
