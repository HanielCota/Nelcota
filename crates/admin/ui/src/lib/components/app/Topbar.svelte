<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import * as Sheet from '$lib/components/ui/sheet'
  import { Button } from '$lib/components/ui/button'
  import { mode, toggleMode } from 'mode-watcher'
  import Menu from '@lucide/svelte/icons/menu'
  import Sun from '@lucide/svelte/icons/sun'
  import Moon from '@lucide/svelte/icons/moon'
  import BookOpen from '@lucide/svelte/icons/book-open'
  import LogOut from '@lucide/svelte/icons/log-out'
  import Logo from './Logo.svelte'
  import ProjectSwitcher from './ProjectSwitcher.svelte'
  import { api } from '$lib/api'
  import { session } from '$lib/session.svelte'
  import { href, match, route } from '$lib/router.svelte'
  import { isActive, navGroups } from '$lib/nav'

  const titles: Record<string, string> = {
    '/': 'Visão geral',
    '/tables': 'Tabelas',
    '/sql': 'Editor SQL',
    '/users': 'Usuários',
    '/policies': 'Policies',
    '/projects': 'Projetos',
  }

  const crumbs = $derived.by((): { label: string; path?: string }[] => {
    const table = match('/tables/:name', route.path)
    if (table) return [{ label: 'Tabelas', path: '/tables' }, { label: table.name }]
    return [{ label: titles[route.path.replace(/\/$/, '') || '/'] ?? 'Página' }]
  })

  let mobileOpen = $state(false)
  const initial = $derived((session.email ?? '?').charAt(0).toUpperCase())

  async function logout() {
    await api.post('/logout').catch(() => {})
    session.email = null
  }
</script>

{#snippet slash()}
  <svg viewBox="0 0 24 24" class="size-4 shrink-0 text-border-strong" aria-hidden="true">
    <path d="M16 3 8 21" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" fill="none" />
  </svg>
{/snippet}

<header class="flex h-12 shrink-0 items-center gap-1.5 border-b border-sidebar-border bg-sidebar pr-3">
  <div class="flex w-14 shrink-0 justify-center">
    <a href={href('/')} class="hidden md:block" aria-label="Início"><Logo mark /></a>
    <Button variant="ghost" size="icon-sm" class="md:hidden" onclick={() => (mobileOpen = true)} aria-label="Menu">
      <Menu />
    </Button>
  </div>

  <ProjectSwitcher {slash} />

  <nav class="flex min-w-0 items-center gap-1.5 text-sm">
    {#each crumbs as crumb, i (i)}
      {#if i > 0}{@render slash()}{/if}
      {#if crumb.path}
        <a href={href(crumb.path)} class="truncate text-muted-foreground transition-colors hover:text-foreground"
          >{crumb.label}</a
        >
      {:else}
        <span class="truncate text-foreground">{crumb.label}</span>
      {/if}
    {/each}
  </nav>

  <div class="ml-auto flex items-center gap-1.5">
    <Button variant="outline" size="sm" href="/rest/v1/" target="_blank" rel="noopener" class="hidden sm:inline-flex">
      <BookOpen />API
    </Button>
    <Button variant="ghost" size="icon-sm" onclick={toggleMode} aria-label="Alternar tema" title="Alternar tema">
      {#if mode.current === 'dark'}<Sun />{:else}<Moon />{/if}
    </Button>
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <button
            {...props}
            class="ml-1 grid size-7 place-items-center rounded-full border border-border-strong bg-muted text-xs font-medium text-foreground transition-colors hover:border-brand/60"
            aria-label="Conta"
          >
            {initial}
          </button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-60">
        <div class="px-2 py-1.5">
          <p class="text-xs font-light text-muted-foreground">Conectado como</p>
          <p class="truncate text-sm font-medium">{session.email}</p>
        </div>
        <DropdownMenu.Separator />
        <DropdownMenu.Item onclick={toggleMode}>
          {#if mode.current === 'dark'}<Sun />{:else}<Moon />{/if}Alternar tema
        </DropdownMenu.Item>
        <DropdownMenu.Item onclick={logout}><LogOut />Sair</DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </div>
</header>

<!-- Navegação em telas pequenas, onde o trilho lateral some. -->
<Sheet.Root bind:open={mobileOpen}>
  <Sheet.Content side="left" class="w-64 gap-0 bg-sidebar p-0">
    <div class="flex h-12 items-center border-b border-sidebar-border px-4"><Logo /></div>
    <nav class="p-2">
      {#each navGroups as group, g (g)}
        {#if g > 0}<div class="mx-2 my-2 border-t border-sidebar-border"></div>{/if}
        {#each group as item (item.path)}
          {@const active = isActive(item.path)}
          <a
            href={href(item.path)}
            onclick={() => (mobileOpen = false)}
            class={[
              'flex h-9 items-center gap-3 rounded-md px-3 text-sm',
              active ? 'bg-sidebar-accent text-foreground' : 'text-sidebar-foreground hover:bg-sidebar-accent/60',
            ]}
          >
            <item.icon class={['size-[18px]', active && 'text-brand']} strokeWidth={1.6} />{item.title}
          </a>
        {/each}
      {/each}
    </nav>
  </Sheet.Content>
</Sheet.Root>
