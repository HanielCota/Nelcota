<script lang="ts">
  import * as Sidebar from '$lib/components/ui/sidebar'
  import * as Breadcrumb from '$lib/components/ui/breadcrumb'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { toggleMode } from 'mode-watcher'
  import { api } from '$lib/api'
  import { session } from '$lib/session.svelte'
  import { href, match, route } from '$lib/router.svelte'

  const titles: Record<string, string> = {
    '/': 'Visão geral',
    '/tables': 'Tabelas',
    '/sql': 'SQL',
    '/users': 'Usuários',
    '/policies': 'Policies',
  }

  const crumbs = $derived.by((): { label: string; path?: string }[] => {
    const table = match('/tables/:name', route.path)
    if (table) return [{ label: 'Tabelas', path: '/tables' }, { label: table.name }]
    return [{ label: titles[route.path.replace(/\/$/, '') || '/'] ?? 'Página' }]
  })

  async function logout() {
    await api.post('/logout').catch(() => {})
    session.email = null
  }
</script>

<header class="flex h-12 shrink-0 items-center gap-3 border-b px-3">
  <Sidebar.Trigger />
  <Breadcrumb.Root>
    <Breadcrumb.List>
      {#each crumbs as crumb, i (i)}
        {#if i > 0}<Breadcrumb.Separator />{/if}
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

  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <Button variant="ghost" size="sm" class="ml-auto text-xs text-muted-foreground" {...props}>
          {session.email}
        </Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="end" class="w-44">
      <DropdownMenu.Item onclick={toggleMode}>Alternar tema</DropdownMenu.Item>
      <DropdownMenu.Separator />
      <DropdownMenu.Item onclick={logout}>Sair</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
</header>
