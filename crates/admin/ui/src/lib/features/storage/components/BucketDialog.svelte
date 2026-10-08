<script lang="ts">
  import { untrack } from 'svelte'
  import { CloseGuard } from '$lib/close-guard.svelte'
  import UnsavedChangesDialog from '$lib/components/shared/UnsavedChangesDialog.svelte'
  import Save from '@lucide/svelte/icons/save'
  import HardDrive from '@lucide/svelte/icons/hard-drive'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import { toast } from 'svelte-sonner'
  import { api, enc } from '$lib/api'
  import { formatBytes, parseTypes, validBucketName } from '$lib/features/storage/files'
  import type { Bucket } from '$lib/types'
  import { errorMessage, intlLocale, t } from '$lib/i18n/index.svelte'

  // Creates a bucket, or edits one when `bucket` is given.
  let {
    open = $bindable(false),
    bucket = null,
    serverLimit,
    onsaved,
  }: { open?: boolean; bucket?: Bucket | null; serverLimit: number; onsaved: () => void } = $props()

  const MB = 1024 * 1024

  let id = $state('')
  let isPublic = $state(false)
  let limitMb = $state('')
  let types = $state('')
  let saving = $state(false)
  let initialValue = $state('')
  const snapshot = () => JSON.stringify([id, isPublic, limitMb, types])
  const guard = new CloseGuard(() => snapshot() !== initialValue, () => saving, () => (open = false))

  $effect(() => {
    if (open) {
      untrack(() => {
        id = bucket?.id ?? ''
        isPublic = bucket?.public ?? false
        limitMb = bucket?.file_size_limit ? String(+(bucket.file_size_limit / MB).toFixed(2)) : ''
        types = bucket?.allowed_mime_types?.join(', ') ?? ''
        initialValue = snapshot()
        })
    }
  })

  const limit = $derived(limitMb.trim() === '' ? null : Math.round(Number(limitMb.replace(',', '.')) * MB))
  const limitOk = $derived(limit === null || (Number.isFinite(limit) && limit > 0))
  const ready = $derived((bucket !== null || validBucketName(id)) && limitOk)

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (saving || !ready) return
    saving = true
    const settings = { public: isPublic, file_size_limit: limit, allowed_mime_types: parseTypes(types) }
    try {
      if (bucket) {
        await api.put(`/storage/buckets/${enc(bucket.id)}`, settings)
        toast.success(t('storage.dialog.saved', { bucket: bucket.id }))
      } else {
        await api.post('/storage/buckets', { id, ...settings })
        toast.success(t('storage.dialog.created', { bucket: id }))
      }
      open = false
      onsaved()
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      saving = false
    }
  }
</script>

<Dialog.Root bind:open={() => open, guard.change}>
  <Dialog.Content class="sm:max-w-lg">
    <form class="grid gap-5" onsubmit={submit}>
      <fieldset class="contents" disabled={saving} aria-busy={saving}>
        <Dialog.Header>
          <Dialog.Title>
            {bucket ? t('storage.dialog.editTitle', { bucket: bucket.id }) : t('storage.dialog.createTitle')}
          </Dialog.Title>
        </Dialog.Header>
        {#if !bucket}
          <div class="grid gap-2">
            <Label for="bucket-id">{t('storage.dialog.name')}</Label>
            <Input
              id="bucket-id"
              bind:value={id}
              placeholder="avatars"
              autocomplete="off"
              spellcheck={false}
              class="h-10 font-mono"
              aria-invalid={id !== '' && !validBucketName(id)}
              required
            />
            <p class="text-xs text-muted-foreground">{t('storage.dialog.nameHint')}</p>
          </div>
        {/if}
        <div class="grid gap-1.5">
          <label class="flex cursor-pointer items-center gap-2 text-sm font-medium">
            <Checkbox bind:checked={isPublic} />{t('storage.dialog.public')}
          </label>
          <p class="text-xs text-muted-foreground">{t('storage.dialog.publicHint')}</p>
        </div>
        <div class="grid gap-2">
          <Label for="bucket-limit">{t('storage.dialog.limit')}</Label>
          <Input
            id="bucket-limit"
            bind:value={limitMb}
            inputmode="decimal"
            autocomplete="off"
            class="h-10 w-40 font-mono"
            aria-invalid={!limitOk}
          />
          <p class="text-xs text-muted-foreground">
            {t('storage.dialog.limitHint', { size: formatBytes(serverLimit, intlLocale()) })}
          </p>
        </div>
        <div class="grid gap-2">
          <Label for="bucket-types">{t('storage.dialog.types')}</Label>
          <Input id="bucket-types" bind:value={types} placeholder="image/*" autocomplete="off" spellcheck={false} class="h-10 font-mono" />
          <p class="text-xs text-muted-foreground">{t('storage.dialog.typesHint')}</p>
        </div>
        <Dialog.Footer>
          <Button variant="outline" disabled={saving} onclick={guard.request}>{t('common.cancel')}</Button>
          <Button type="submit" disabled={saving || !ready}>
            {#if saving}<LoaderCircle data-icon="inline-start" class="animate-spin" aria-hidden="true" />{:else if bucket}<Save data-icon="inline-start" aria-hidden="true" />{:else}<HardDrive data-icon="inline-start" aria-hidden="true" />{/if}
            {saving ? t('common.saving') : bucket ? t('storage.dialog.save') : t('storage.dialog.create')}
          </Button>
        </Dialog.Footer>
        </fieldset>
    </form>
  </Dialog.Content>
</Dialog.Root>
<UnsavedChangesDialog bind:open={guard.pending} ondiscard={guard.discard} />
