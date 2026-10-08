<script lang="ts">
  import { onMount } from 'svelte'
  import { ModeWatcher } from 'mode-watcher'
  import { Toaster } from '$lib/components/ui/sonner'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import AppSidebar from '$lib/shell/components/AppSidebar.svelte'
  import Topbar from '$lib/shell/components/Topbar.svelte'
  import CommandPalette from '$lib/shell/components/CommandPalette.svelte'
  import Login from '$lib/features/auth/Login.svelte'
  import Overview from '$lib/features/overview/Overview.svelte'
  import TableEditor from '$lib/features/tables/TableEditor.svelte'
  import Users from '$lib/features/users/Users.svelte'
  import Storage from '$lib/features/storage/Storage.svelte'
  import StorageBucket from '$lib/features/storage/StorageBucket.svelte'
  import Policies from '$lib/features/policies/Policies.svelte'
  import Migrations from '$lib/features/migrations/Migrations.svelte'
  import NotFound from '$lib/shell/NotFound.svelte'
  import Projects from '$lib/features/projects/Projects.svelte'
  import ApiPage from '$lib/features/api/ApiPage.svelte'
  import { takeHandoffToken } from '$lib/features/projects/projects'
  import { api } from '$lib/api'
  import { session } from '$lib/features/auth/session.svelte'
  import { match, route } from '$lib/router.svelte'
  import { crumbsFor, documentTitle } from '$lib/shell/titles'
  import { t } from '$lib/i18n/index.svelte'

  onMount(async () => {
    // Coming from another panel on the host (single sign-on): trade the token for a session.
    const token = takeHandoffToken()
    if (token) {
      try {
        session.email = (await api.post<{ email: string }>('/sso', { token })).email
        return
      } catch {
        // Expired or already used token: fall back to the normal login.
      }
    }
    try {
      const me = await api.get<{ email: string }>('/session')
      session.email = me.email
    } catch {
      session.email = null
    }
  })

  const path = $derived(route.path.replace(/\/$/, '') || '/')
  const structureName = $derived(match('/tables/:name/structure', path)?.name)
  const tableName = $derived(match('/tables/:name', path)?.name ?? structureName)
  // Switching tabs (Data/Structure) keeps the editor mounted; switching tables
  // does not (sort, page and selection belong to each table).
  const bucketName = $derived(match('/storage/:bucket', path)?.bucket)
  const pageKey = $derived(tableName ? `/tables/${tableName}` : route.path)

  // Tab title: the current page (or the login) followed by the product name.
  const title = $derived(
    session.email === null ? `${t('shell.pages.signIn')} · Nelcota` : session.email ? documentTitle(crumbsFor(route.path)) : 'Nelcota',
  )
</script>

<svelte:head><title>{title}</title></svelte:head>

<ModeWatcher defaultMode="dark" />
<Toaster position="bottom-right" />

{#if session.email === undefined}
  <div class="grid h-screen place-items-center">
    <div class="flex flex-col items-center gap-4" role="status">
      <span class="size-7 animate-spin rounded-full border-[2.5px] border-border-strong border-t-brand"></span>
      <span class="text-sm font-medium text-muted-foreground">{t('shell.app.loading')}</span>
    </div>
  </div>
{:else if session.email === null}
  <Login />
{:else}
  <Tooltip.Provider delayDuration={200}>
    <!-- First Tab stop: skips the topbar and menu. -->
    <a
      href="#conteudo"
      class="sr-only z-50 rounded-lg bg-primary px-4 py-2.5 text-sm font-semibold text-primary-foreground shadow-raised focus:not-sr-only focus:fixed focus:top-3 focus:left-3"
      >{t('shell.app.skipToContent')}</a
    >
    <!-- Full-height sidebar on the left; topbar and page on the right. -->
    <div class="flex h-screen overflow-hidden bg-background">
      <AppSidebar />
      <div class="flex min-w-0 flex-1 flex-col">
        <Topbar />
        <main id="conteudo" tabindex="-1" class="min-h-0 min-w-0 flex-1 overflow-auto outline-none">
          {#key pageKey}
            {#if path === '/'}
              <Overview />
            {:else if path === '/tables' || tableName}
              <TableEditor name={tableName} view={structureName ? 'structure' : 'data'} />
            {:else if path === '/sql'}
              <!-- CodeMirror is only downloaded when the SQL editor opens. -->
              {#await import('$lib/features/sql/SqlEditor.svelte') then m}
                <m.default />
              {/await}
            {:else if path === '/migrations'}
              <Migrations />
            {:else if path === '/users'}
              <Users />
            {:else if path === '/storage'}
              <Storage />
            {:else if bucketName}
              <StorageBucket bucket={bucketName} />
            {:else if path === '/policies'}
              <Policies />
            {:else if path === '/projects'}
              <Projects />
            {:else if path === '/connect'}
              <ApiPage />
            {:else}
              <NotFound />
            {/if}
          {/key}
        </main>
      </div>
    </div>
    <CommandPalette />
  </Tooltip.Provider>
{/if}
