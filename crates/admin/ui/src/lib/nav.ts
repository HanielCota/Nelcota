import type { Component } from 'svelte'
import House from '@lucide/svelte/icons/house'
import Table2 from '@lucide/svelte/icons/table-2'
import SquareTerminal from '@lucide/svelte/icons/square-terminal'
import History from '@lucide/svelte/icons/history'
import Users from '@lucide/svelte/icons/users'
import ShieldCheck from '@lucide/svelte/icons/shield-check'
import Boxes from '@lucide/svelte/icons/boxes'
import Plug from '@lucide/svelte/icons/plug'
import { route } from './router.svelte'

export type NavItem = { title: string; path: string; icon: Component }
export type NavGroup = { label?: string; items: NavItem[] }

// Grupos com título na barra lateral (o primeiro não tem).
export const navGroups: NavGroup[] = [
  { items: [{ title: 'Visão geral', path: '/', icon: House }] },
  {
    label: 'Banco de dados',
    items: [
      { title: 'Tabelas', path: '/tables', icon: Table2 },
      { title: 'Editor SQL', path: '/sql', icon: SquareTerminal },
      { title: 'Migrações', path: '/migrations', icon: History },
    ],
  },
  {
    label: 'Acesso',
    items: [
      { title: 'Usuários', path: '/users', icon: Users },
      { title: 'Policies', path: '/policies', icon: ShieldCheck },
    ],
  },
  {
    label: 'Integração',
    // /connect, não /api: /admin/api/* é o prefixo da API do painel.
    items: [{ title: 'API', path: '/connect', icon: Plug }],
  },
]

// Projetos fica fora do menu: o seletor de projeto, no topo da barra, já leva lá.
export const navItems: NavItem[] = [
  ...navGroups.flatMap((group) => group.items),
  { title: 'Projetos', path: '/projects', icon: Boxes },
]

export const isActive = (path: string) =>
  path === '/' ? route.path === '/' : route.path === path || route.path.startsWith(path + '/')
