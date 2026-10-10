<script lang="ts">
  import { avatarUrl, profile } from '$lib/features/profile/profile.svelte'
  import { session } from '$lib/features/auth/session.svelte'
  import { cn } from '$lib/utils'

  // Photo or initials. `src` also supports the unsaved photo preview.
  let { src = null, class: className = '' }: { src?: string | null; class?: string } = $props()

  const initial = $derived(
    ((session.email ?? '?').split('@')[0]!.replace(/[^\p{L}\p{N}]/gu, '') || '?').slice(0, 2).toUpperCase(),
  )
  const url = $derived(src ?? (profile.avatar !== null ? avatarUrl(profile.avatar) : null))
  // Image that failed to load (removed in another tab, for example): show the initial.
  let failed = $state<string | null>(null)
</script>

<span
  class={cn(
    'grid shrink-0 place-items-center overflow-hidden rounded-full bg-muted font-semibold text-foreground',
    className,
  )}
>
  {#if url && failed !== url}
    <img src={url} alt="" class="size-full object-cover" draggable="false" onerror={() => (failed = url)} />
  {:else}
    {initial}
  {/if}
</span>
