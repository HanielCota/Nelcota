<script lang="ts">
  import { onMount } from 'svelte'
  import { ModeWatcher } from 'mode-watcher'
  import { Toaster } from '$lib/components/ui/sonner'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import AppSidebar from '$lib/components/app/AppSidebar.svelte'
  import Topbar from '$lib/components/app/Topbar.svelte'
  import CommandPalette from '$lib/components/app/CommandPalette.svelte'
  import Login from '$lib/pages/Login.svelte'
  import Overview from '$lib/pages/Overview.svelte'
  import TableEditor from '$lib/pages/TableEditor.svelte'
  import Users from '$lib/pages/Users.svelte'
  import Policies from '$lib/pages/Policies.svelte'
  import NotFound from '$lib/pages/NotFound.svelte'
  import Projects from '$lib/pages/Projects.svelte'
  import { takeHandoffToken } from '$lib/projects'
  import { api } from '$lib/api'
  import { session } from '$lib/session.svelte'
  import { match, route } from '$lib/router.svelte'

  onMount(async () => {
    // Vindo de outro painel do host (login único): troca o token por sessão.
    const token = takeHandoffToken()
    if (token) {
      try {
        session.email = (await api.post<{ email: string }>('/sso', { token })).email
        return
      } catch {
        // Token vencido ou já usado: segue para o login normal.
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
  const tableName = $derived(match('/tables/:name', path)?.name)
</script>

<ModeWatcher defaultMode="dark" />
<Toaster position="bottom-right" />

{#if session.email === undefined}
  <div class="grid h-screen place-items-center">
    <span class="size-5 animate-spin rounded-full border-2 border-border-strong border-t-brand"></span>
  </div>
{:else if session.email === null}
  <Login />
{:else}
  <Tooltip.Provider delayDuration={200}>
    <div class="flex h-screen flex-col overflow-hidden bg-background">
      <Topbar />
      <div class="flex min-h-0 flex-1">
        <AppSidebar />
        <main class="min-w-0 flex-1 overflow-auto">
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
            {:else if path === '/projects'}
              <Projects />
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
