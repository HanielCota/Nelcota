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
  import ApiPage from '$lib/pages/ApiPage.svelte'
  import { takeHandoffToken } from '$lib/projects'
  import { api } from '$lib/api'
  import { session } from '$lib/session.svelte'
  import { match, route } from '$lib/router.svelte'
  import { crumbsFor, documentTitle } from '$lib/titles'

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
  const structureName = $derived(match('/tables/:name/structure', path)?.name)
  const tableName = $derived(match('/tables/:name', path)?.name ?? structureName)
  // Trocar de aba (Dados/Estrutura) não remonta o editor; trocar de tabela sim
  // (ordenação, página e seleção são de cada tabela).
  const pageKey = $derived(tableName ? `/tables/${tableName}` : route.path)

  // Título da aba: a página atual (ou o login) seguida do nome do produto.
  const title = $derived(
    session.email === null ? 'Entrar · Nelcota' : session.email ? documentTitle(crumbsFor(route.path)) : 'Nelcota',
  )
</script>

<svelte:head><title>{title}</title></svelte:head>

<ModeWatcher defaultMode="dark" />
<Toaster position="bottom-right" />

{#if session.email === undefined}
  <div class="grid h-screen place-items-center">
    <div class="flex flex-col items-center gap-4" role="status">
      <span class="size-7 animate-spin rounded-full border-[2.5px] border-border-strong border-t-brand"></span>
      <span class="text-sm font-medium text-muted-foreground">Carregando painel…</span>
    </div>
  </div>
{:else if session.email === null}
  <Login />
{:else}
  <Tooltip.Provider delayDuration={200}>
    <!-- Primeira parada do Tab: pula topbar e menu. -->
    <a
      href="#conteudo"
      class="sr-only z-50 rounded-lg bg-primary px-4 py-2.5 text-sm font-semibold text-primary-foreground shadow-raised focus:not-sr-only focus:fixed focus:top-3 focus:left-3"
      >Pular para o conteúdo</a
    >
    <!-- Barra lateral de altura total à esquerda; topbar e página à direita. -->
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
