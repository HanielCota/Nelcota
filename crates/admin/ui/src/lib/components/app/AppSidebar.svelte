<script lang="ts">
  import * as Sidebar from '$lib/components/ui/sidebar'
  import LayoutDashboard from '@lucide/svelte/icons/layout-dashboard'
  import Table2 from '@lucide/svelte/icons/table-2'
  import SquareTerminal from '@lucide/svelte/icons/square-terminal'
  import Users from '@lucide/svelte/icons/users'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import BookOpen from '@lucide/svelte/icons/book-open'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import Logo from './Logo.svelte'
  import { href, route } from '$lib/router.svelte'

  const groups = [
    {
      label: 'Projeto',
      items: [
        { title: 'Visão geral', path: '/', icon: LayoutDashboard },
        { title: 'Editor de tabelas', path: '/tables', icon: Table2 },
        { title: 'Editor SQL', path: '/sql', icon: SquareTerminal },
      ],
    },
    {
      label: 'Autenticação',
      items: [
        { title: 'Usuários', path: '/users', icon: Users },
        { title: 'Policies', path: '/policies', icon: ShieldCheck },
      ],
    },
  ]

  const isActive = (path: string) =>
    path === '/' ? route.path === '/' : route.path === path || route.path.startsWith(path + '/')
</script>

<Sidebar.Root collapsible="icon">
  <Sidebar.Header>
    <Sidebar.Menu>
      <Sidebar.MenuItem>
        <Sidebar.MenuButton size="lg" class="hover:bg-transparent">
          {#snippet child({ props })}
            <a href={href('/')} {...props}>
              <Logo class="size-8! shrink-0" />
              <div class="grid flex-1 text-left leading-tight">
                <span class="truncate font-semibold">nelcota</span>
                <span class="truncate text-xs text-muted-foreground">painel administrativo</span>
              </div>
            </a>
          {/snippet}
        </Sidebar.MenuButton>
      </Sidebar.MenuItem>
    </Sidebar.Menu>
  </Sidebar.Header>

  <Sidebar.Content>
    {#each groups as group (group.label)}
      <Sidebar.Group>
        <Sidebar.GroupLabel>{group.label}</Sidebar.GroupLabel>
        <Sidebar.GroupContent>
          <Sidebar.Menu>
            {#each group.items as item (item.path)}
              <Sidebar.MenuItem>
                <Sidebar.MenuButton isActive={isActive(item.path)} tooltipContent={item.title}>
                  {#snippet child({ props })}
                    <a href={href(item.path)} {...props}>
                      <item.icon class={isActive(item.path) ? 'text-primary' : ''} />
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

  <Sidebar.Footer>
    <div
      class="flex items-start gap-2 rounded-md border border-warning/25 bg-warning/5 p-2 text-[11px] leading-snug text-muted-foreground group-data-[collapsible=icon]:hidden"
    >
      <KeyRound class="mt-0.5 size-3.5 shrink-0 text-warning" />
      <span
        >A chave <code class="font-mono text-foreground">service_role</code> ignora o RLS: use só no seu
        backend.</span
      >
    </div>
  </Sidebar.Footer>
  <Sidebar.Rail />
</Sidebar.Root>
