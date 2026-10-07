<script lang="ts">
  import { avatarUrl, profile } from '$lib/profile.svelte'
  import { session } from '$lib/session.svelte'
  import { cn } from '$lib/utils'

  // The admin's photo or, without one, the email's initial. `src` forces an
  // image (preview of a photo not saved yet).
  let { src = null, class: className = '' }: { src?: string | null; class?: string } = $props()

  const initial = $derived((session.email ?? '?').charAt(0).toUpperCase())
  const url = $derived(src ?? (profile.avatar !== null ? avatarUrl(profile.avatar) : null))
  // Image that failed to load (removed in another tab, for example): show the initial.
  let failed = $state<string | null>(null)
</script>

<span
  class={cn(
    'grid shrink-0 place-items-center overflow-hidden rounded-full border border-border-strong bg-muted font-medium text-foreground',
    className,
  )}
>
  {#if url && failed !== url}
    <img src={url} alt="" class="size-full object-cover" draggable="false" onerror={() => (failed = url)} />
  {:else}
    {initial}
  {/if}
</span>
