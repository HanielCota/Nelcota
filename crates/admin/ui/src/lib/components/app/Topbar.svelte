<script lang="ts">
  import * as Sidebar from '$lib/components/ui/sidebar'
  import * as Breadcrumb from '$lib/components/ui/breadcrumb'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { Separator } from '$lib/components/ui/separator'
  import Moon from '@lucide/svelte/icons/moon'
  import Sun from '@lucide/svelte/icons/sun'
  import LogOut from '@lucide/svelte/icons/log-out'
  import { toggleMode } from 'mode-watcher'
  import { api } from '$lib/api'
  import { session } from '$lib/session.svelte'
  import { href, match, route } from '$lib/router.svelte'

  const titles: Record<string, string> = {
    '/': 'Visão geral',
    '/tables': 'Editor de tabelas',
    '/sql': 'Editor SQL',
    '/users': 'Usuários',
    '/policies': 'Policies',
  }

  const crumbs = $derived.by((): { label: string; path?: string }[] => {
    const table = match('/tables/:name', route.path)
    if (table) return [{ label: 'Editor de tabelas', path: '/tables' }, { label: table.name }]
    return [{ label: titles[route.path.replace(/\/$/, '') || '/'] ?? 'Página' }]
  })

  async function logout() {
    await api.post('/logout').catch(() => {})
    session.email = null
  }
</script>

<header class="flex h-12 shrink-0 items-center gap-2 border-b px-3">
  <Sidebar.Trigger class="-ml-1" />
  <Separator orientation="vertical" class="mr-1 data-[orientation=vertical]:h-4" />
  <Breadcrumb.Root>
    <Breadcrumb.List>
      <Breadcrumb.Item class="hidden sm:block">
        <Breadcrumb.Link href={href('/')}>nelcota</Breadcrumb.Link>
      </Breadcrumb.Item>
      {#each crumbs as crumb, i (i)}
        <Breadcrumb.Separator class="hidden sm:block" />
        <Breadcrumb.Item>
          {#if crumb.path}
            <Breadcrumb.Link href={href(crumb.path)}>{crumb.label}</Breadcrumb.Link>
          {:else}
            <Breadcrumb.Page>{crumb.label}</Breadcrumb.Page>
          {/if}
        </Breadcrumb.Item>
      {/each}
    </Breadcrumb.List>
  </Breadcrumb.Root>

  <div class="ml-auto flex items-center gap-1">
    <span
      class="mr-2 hidden items-center gap-1.5 rounded-full border border-primary/30 bg-primary/10 px-2 py-0.5 text-[11px] font-medium text-primary md:inline-flex"
    >
      <span class="size-1.5 rounded-full bg-primary"></span>Postgres 17
    </span>
    <Button variant="ghost" size="icon-sm" onclick={toggleMode} aria-label="Alternar tema">
      <Sun class="hidden dark:block" />
      <Moon class="dark:hidden" />
    </Button>
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Button variant="ghost" size="sm" class="gap-2" {...props}>
            <span
              class="grid size-6 place-items-center rounded-full bg-primary/15 text-[11px] font-semibold text-primary"
              >{session.email?.[0]?.toUpperCase()}</span
            >
            <span class="hidden max-w-40 truncate text-xs md:inline">{session.email}</span>
          </Button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-56">
        <DropdownMenu.Label class="truncate font-normal text-muted-foreground">{session.email}</DropdownMenu.Label>
        <DropdownMenu.Separator />
        <DropdownMenu.Item onclick={logout}>
          <LogOut />Sair
        </DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </div>
</header>
