<script lang="ts">
  import { tick } from 'svelte'
  import { Toaster } from '$lib/components/ui/sonner'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import TopNav from './components/TopNav.svelte'
  import CommandPalette from './components/CommandPalette.svelte'
  import NotFound from './NotFound.svelte'
  import { session } from '$lib/features/auth/session.svelte'
  import { match, route } from '$lib/router.svelte'
  import { crumbsFor } from './titles'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  const path = $derived(route.path.replace(/\/$/, '') || '/')
  const structureName = $derived(match('/tables/:name/structure', path)?.name)
  const tableName = $derived(match('/tables/:name', path)?.name ?? structureName)
  const bucketName = $derived(match('/storage/:bucket', path)?.bucket)
  // Data/Structure keeps the editor mounted; a different table gets fresh state.
  const pageKey = $derived(tableName ? `/tables/${tableName}` : route.path)
  let content = $state<HTMLElement>()
  let previousPath = route.path
  const pageLabel = $derived(crumbsFor(route.path).map((crumb) => crumb.label).join(' · '))

  $effect(() => {
    const nextPath = route.path
    if (nextPath === previousPath) return
    previousPath = nextPath
    if (session.email) {
      void tick().then(() => {
        if (route.path === nextPath) content?.focus()
      })
    }
  })
</script>

{#snippet loading()}
  <p class="p-6 text-sm text-muted-foreground" role="status">{t('shell.app.loading')}</p>
{/snippet}
{#snippet failed(error: unknown)}
  <div class="p-6 text-sm text-destructive" role="alert">
    <p>{errorMessage(error)}</p>
    <button class="mt-3 underline" onclick={() => window.location.reload()}>{t('common.retry')}</button>
  </div>
{/snippet}

<Toaster position="bottom-right" />
<Tooltip.Provider delayDuration={200}>
  <a
    href="#conteudo"
    class="sr-only z-50 rounded-lg bg-primary px-4 py-2.5 text-sm font-semibold text-primary-foreground shadow-raised focus:not-sr-only focus:fixed focus:top-3 focus:left-3"
    >{t('shell.app.skipToContent')}</a
  >
  <div class="flex h-dvh flex-col gap-4 overflow-hidden bg-background sm:gap-6">
    <TopNav />
    <div class="flex min-h-0 min-w-0 flex-1 flex-col">
      <!-- Route and skip-link focus identify the page without framing the viewport. -->
      <!-- Keep absolute accessibility labels inside this scroll container. -->
      <main bind:this={content} id="conteudo" tabindex="-1" aria-label={pageLabel} class="relative min-h-0 min-w-0 flex-1 overflow-auto outline-none">
        {#key pageKey}
          {#if path === '/'}
            {#await import('$lib/features/overview/Overview.svelte')}
              {@render loading()}
            {:then m}<m.default />{:catch error}{@render failed(error)}{/await}
          {:else if path === '/tables' || tableName}
            {#await import('$lib/features/tables/TableEditor.svelte')}
              {@render loading()}
            {:then m}<m.default name={tableName} view={structureName ? 'structure' : 'data'} />{:catch error}{@render failed(error)}{/await}
          {:else if path === '/sql'}
            {#await import('$lib/features/sql/SqlEditor.svelte')}
              {@render loading()}
            {:then m}<m.default />{:catch error}{@render failed(error)}{/await}
          {:else if path === '/migrations'}
            {#await import('$lib/features/migrations/Migrations.svelte')}
              {@render loading()}
            {:then m}<m.default />{:catch error}{@render failed(error)}{/await}
          {:else if path === '/users'}
            {#await import('$lib/features/users/Users.svelte')}
              {@render loading()}
            {:then m}<m.default />{:catch error}{@render failed(error)}{/await}
          {:else if path === '/sign-in'}
            {#await import('$lib/features/sign-in/SignIn.svelte')}
              {@render loading()}
            {:then m}<m.default />{:catch error}{@render failed(error)}{/await}
          {:else if path === '/storage'}
            {#await import('$lib/features/storage/Storage.svelte')}
              {@render loading()}
            {:then m}<m.default />{:catch error}{@render failed(error)}{/await}
          {:else if bucketName}
            {#await import('$lib/features/storage/StorageBucket.svelte')}
              {@render loading()}
            {:then m}<m.default bucket={bucketName} />{:catch error}{@render failed(error)}{/await}
          {:else if path === '/policies'}
            {#await import('$lib/features/policies/Policies.svelte')}
              {@render loading()}
            {:then m}<m.default />{:catch error}{@render failed(error)}{/await}
          {:else if path === '/projects'}
            {#await import('$lib/features/projects/Projects.svelte')}
              {@render loading()}
            {:then m}<m.default />{:catch error}{@render failed(error)}{/await}
          {:else if path === '/connect'}
            {#await import('$lib/features/api/ApiPage.svelte')}
              {@render loading()}
            {:then m}<m.default />{:catch error}{@render failed(error)}{/await}
          {:else}<NotFound />{/if}
        {/key}
      </main>
    </div>
  </div>
  <CommandPalette />
</Tooltip.Provider>
