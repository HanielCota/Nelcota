<script lang="ts">
  import { onMount } from 'svelte'
  import { ModeWatcher } from 'mode-watcher'
  import { Toaster } from '$lib/components/ui/sonner'
  import * as Sidebar from '$lib/components/ui/sidebar'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import AppSidebar from '$lib/components/app/AppSidebar.svelte'
  import Topbar from '$lib/components/app/Topbar.svelte'
  import Login from '$lib/pages/Login.svelte'
  import Overview from '$lib/pages/Overview.svelte'
  import TableEditor from '$lib/pages/TableEditor.svelte'
  import Users from '$lib/pages/Users.svelte'
  import Policies from '$lib/pages/Policies.svelte'
  import NotFound from '$lib/pages/NotFound.svelte'
  import { api } from '$lib/api'
  import { session } from '$lib/session.svelte'
  import { match, route } from '$lib/router.svelte'

  onMount(async () => {
    try {
      const me = await api.get<{ email: string }>('/session')
      session.email = me.email
    } catch {
      session.email = null
    }
  })

  const path = $derived(route.path.replace(/\/$/, '') || '/')
  const tableName = $derived(match('/tables/:name', path)?.name)
</script>

<ModeWatcher defaultMode="dark" />
<Toaster position="bottom-right" richColors closeButton />

{#if session.email === undefined}
  <div class="grid h-screen place-items-center text-sm text-muted-foreground">Carregando…</div>
{:else if session.email === null}
  <Login />
{:else}
  <Tooltip.Provider delayDuration={200}>
    <Sidebar.Provider>
      <AppSidebar />
      <Sidebar.Inset class="flex h-screen min-w-0 flex-col overflow-hidden">
        <Topbar />
        <main class="min-h-0 flex-1 overflow-auto">
          {#key route.path}
            {#if path === '/'}
              <Overview />
            {:else if path === '/tables' || tableName}
              <TableEditor name={tableName} />
            {:else if path === '/sql'}
              <!-- CodeMirror só é baixado quando o editor SQL é aberto. -->
              {#await import('$lib/pages/SqlEditor.svelte') then m}
                <m.default />
              {/await}
            {:else if path === '/users'}
              <Users />
            {:else if path === '/policies'}
              <Policies />
            {:else}
              <NotFound />
            {/if}
          {/key}
        </main>
      </Sidebar.Inset>
    </Sidebar.Provider>
  </Tooltip.Provider>
{/if}
