<script lang="ts">
  import * as Sheet from '$lib/components/ui/sheet'
  import { Button } from '$lib/components/ui/button'
  import Menu from '@lucide/svelte/icons/menu'
  import Plug from '@lucide/svelte/icons/plug'
  import Search from '@lucide/svelte/icons/search'
  import Logo from './Logo.svelte'
  import ProjectSwitcher from './ProjectSwitcher.svelte'
  import AccountMenu from './AccountMenu.svelte'
  import { palette } from '$lib/palette.svelte'
  import { href, match, route } from '$lib/router.svelte'
  import { isActive, navGroups } from '$lib/nav'

  // Barra do topo da área de conteúdo: trilha e busca. Projeto, páginas e conta
  // ficam na barra lateral (no celular, no menu que desliza da esquerda).

  const titles: Record<string, string> = {
    '/': 'Visão geral',
    '/tables': 'Tabelas',
    '/sql': 'Editor SQL',
    '/users': 'Usuários',
    '/policies': 'Policies',
    '/projects': 'Projetos',
    '/connect': 'API',
  }

  const crumbs = $derived.by((): { label: string; path?: string }[] => {
    const table = match('/tables/:name', route.path)
    if (table) return [{ label: 'Tabelas', path: '/tables' }, { label: table.name }]
    const structure = match('/tables/:name/structure', route.path)
    if (structure) {
      return [
        { label: 'Tabelas', path: '/tables' },
        { label: structure.name, path: `/tables/${encodeURIComponent(structure.name)}` },
        { label: 'Estrutura' },
      ]
    }
    return [{ label: titles[route.path.replace(/\/$/, '') || '/'] ?? 'Página' }]
  })

  let mobileOpen = $state(false)
  const isMac = /Mac|iPhone|iPad/.test(navigator.platform)
</script>

<header class="flex h-14 shrink-0 items-center gap-2 border-b border-sidebar-border bg-sidebar px-3 sm:px-4">
  <div class="flex shrink-0 items-center gap-1 md:hidden">
    <Button variant="ghost" size="icon" onclick={() => (mobileOpen = true)} aria-label="Menu">
      <Menu class="size-5" />
    </Button>
    <a href={href('/')} aria-label="Visão geral"><Logo mark /></a>
  </div>

  <nav class="flex min-w-0 items-center gap-1.5 text-sm font-medium" aria-label="Trilha">
    {#each crumbs as crumb, i (i)}
      {@const last = i === crumbs.length - 1}
      <!-- No celular só a página atual cabe sem cortar. -->
      <span class={['min-w-0 items-center gap-1.5', last ? 'flex' : 'hidden sm:flex']}>
        {#if i > 0}
          <svg viewBox="0 0 24 24" class="hidden size-4 shrink-0 text-border-strong sm:block" aria-hidden="true">
            <path d="M16 3 8 21" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" fill="none" />
          </svg>
        {/if}
        {#if crumb.path}
          <a
            href={href(crumb.path)}
            class="truncate rounded-md px-1 text-muted-foreground transition-colors hover:text-foreground"
            >{crumb.label}</a
          >
        {:else}
          <span class="truncate px-1 text-foreground" aria-current="page">{crumb.label}</span>
        {/if}
      </span>
    {/each}
  </nav>

  <div class="ml-auto flex items-center gap-2">
    <button
      type="button"
      onclick={() => (palette.open = true)}
      class="flex size-9 cursor-pointer items-center justify-center gap-2.5 rounded-md border border-border-strong bg-card text-sm text-muted-foreground transition-colors hover:border-ring hover:text-foreground sm:w-64 sm:justify-start sm:px-3 lg:w-80"
      aria-label="Buscar"
    >
      <Search class="size-4 shrink-0" />
      <span class="hidden flex-1 text-left sm:inline">Buscar…</span>
      <kbd class="hidden rounded border border-border-strong bg-muted px-1.5 font-sans text-2xs sm:inline"
        >{isMac ? '⌘' : 'Ctrl'} K</kbd
      >
    </button>
    <Button variant="outline" href={href('/connect')} class="hidden lg:inline-flex">
      <Plug />Conectar
    </Button>
  </div>
</header>

<!-- Navegação em telas pequenas, onde a barra lateral some. -->
<Sheet.Root bind:open={mobileOpen}>
  <Sheet.Content side="left" class="w-72 gap-0 bg-sidebar p-0">
    <div class="flex h-14 shrink-0 items-center gap-2 border-b border-sidebar-border px-4">
      <Logo mark />
      <div class="min-w-0 flex-1 pr-8"><ProjectSwitcher /></div>
    </div>
    <nav class="grid flex-1 content-start gap-4 overflow-y-auto p-3" aria-label="Navegação principal">
      {#each navGroups as group, g (g)}
        <div>
          {#if group.label}
            <p class="mb-1 px-3 text-xs font-medium text-muted-foreground">{group.label}</p>
          {/if}
          {#each group.items as item (item.path)}
            {@const active = isActive(item.path)}
            <a
              href={href(item.path)}
              onclick={() => (mobileOpen = false)}
              aria-current={active ? 'page' : undefined}
              class={[
                'flex h-10 items-center gap-3 rounded-md px-3 text-sm',
                active ? 'bg-sidebar-accent font-medium text-foreground' : 'text-sidebar-foreground hover:bg-sidebar-accent/60',
              ]}
            >
              <item.icon class={['size-[18px]', active && 'text-brand']} strokeWidth={1.6} />{item.title}
            </a>
          {/each}
        </div>
      {/each}
    </nav>
    <div class="shrink-0 border-t border-sidebar-border p-3"><AccountMenu /></div>
  </Sheet.Content>
</Sheet.Root>
