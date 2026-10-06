<script lang="ts">
  import * as Sidebar from '$lib/components/ui/sidebar'
  import LayoutDashboard from '@lucide/svelte/icons/layout-dashboard'
  import Table2 from '@lucide/svelte/icons/table-2'
  import SquareTerminal from '@lucide/svelte/icons/square-terminal'
  import Users from '@lucide/svelte/icons/users'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import BookOpen from '@lucide/svelte/icons/book-open'
  import Boxes from '@lucide/svelte/icons/boxes'
  import Logo from './Logo.svelte'
  import { href, route } from '$lib/router.svelte'

  // Ícones ficam porque a sidebar recolhe para só ícones.
  const groups = [
    {
      label: '',
      items: [
        { title: 'Visão geral', path: '/', icon: LayoutDashboard },
        { title: 'Tabelas', path: '/tables', icon: Table2 },
        { title: 'SQL', path: '/sql', icon: SquareTerminal },
      ],
    },
    {
      label: 'Autenticação',
      items: [
        { title: 'Usuários', path: '/users', icon: Users },
        { title: 'Policies', path: '/policies', icon: ShieldCheck },
      ],
    },
    {
      label: 'Host',
      items: [{ title: 'Projetos', path: '/projects', icon: Boxes }],
    },
  ]

  const isActive = (path: string) =>
    path === '/' ? route.path === '/' : route.path === path || route.path.startsWith(path + '/')
</script>

<Sidebar.Root collapsible="icon">
  <Sidebar.Header class="h-12 justify-center px-4 group-data-[collapsible=icon]:px-2">
    <a href={href('/')} class="group-data-[collapsible=icon]:hidden"><Logo /></a>
    <a href={href('/')} class="hidden text-center font-semibold group-data-[collapsible=icon]:block">n</a>
  </Sidebar.Header>

  <Sidebar.Content>
    {#each groups as group, g (g)}
      <Sidebar.Group>
        {#if group.label}<Sidebar.GroupLabel>{group.label}</Sidebar.GroupLabel>{/if}
        <Sidebar.GroupContent>
          <Sidebar.Menu>
            {#each group.items as item (item.path)}
              <Sidebar.MenuItem>
                <Sidebar.MenuButton isActive={isActive(item.path)} tooltipContent={item.title}>
                  {#snippet child({ props })}
                    <a href={href(item.path)} {...props}>
                      <item.icon />
                      <span>{item.title}</span>
                    </a>
                  {/snippet}
                </Sidebar.MenuButton>
              </Sidebar.MenuItem>
            {/each}
          </Sidebar.Menu>
        </Sidebar.GroupContent>
      </Sidebar.Group>
    {/each}

    <Sidebar.Group>
      <Sidebar.GroupLabel>API</Sidebar.GroupLabel>
      <Sidebar.GroupContent>
        <Sidebar.Menu>
          <Sidebar.MenuItem>
            <Sidebar.MenuButton tooltipContent="OpenAPI">
              {#snippet child({ props })}
                <a href="/rest/v1/" target="_blank" rel="noopener" {...props}>
                  <BookOpen />
                  <span>OpenAPI</span>
                </a>
              {/snippet}
            </Sidebar.MenuButton>
          </Sidebar.MenuItem>
        </Sidebar.Menu>
      </Sidebar.GroupContent>
    </Sidebar.Group>
  </Sidebar.Content>

  <Sidebar.Footer class="group-data-[collapsible=icon]:hidden">
    <p class="px-2 pb-1 text-[11px] leading-snug text-muted-foreground">
      A chave <span class="font-mono">service_role</span> ignora o RLS. Use só no backend.
    </p>
  </Sidebar.Footer>
  <Sidebar.Rail />
</Sidebar.Root>
