import type { Component } from 'svelte'
import House from '@lucide/svelte/icons/house'
import Table2 from '@lucide/svelte/icons/table-2'
import SquareTerminal from '@lucide/svelte/icons/square-terminal'
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
    label: 'Projeto',
    items: [
      // /connect, não /api: /admin/api/* é o prefixo da API do painel.
      { title: 'API', path: '/connect', icon: Plug },
      { title: 'Projetos', path: '/projects', icon: Boxes },
    ],
  },
]

export const navItems: NavItem[] = navGroups.flatMap((group) => group.items)

export const isActive = (path: string) =>
  path === '/' ? route.path === '/' : route.path === path || route.path.startsWith(path + '/')
