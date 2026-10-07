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

// Grupos separados por uma linha fina na barra lateral.
export const navGroups: NavItem[][] = [
  [{ title: 'Visão geral', path: '/', icon: House }],
  [
    { title: 'Tabelas', path: '/tables', icon: Table2 },
    { title: 'Editor SQL', path: '/sql', icon: SquareTerminal },
  ],
  [
    { title: 'Usuários', path: '/users', icon: Users },
    { title: 'Policies', path: '/policies', icon: ShieldCheck },
  ],
  [
    // /connect, não /api: /admin/api/* é o prefixo da API do painel.
    { title: 'API', path: '/connect', icon: Plug },
    { title: 'Projetos', path: '/projects', icon: Boxes },
  ],
]

export const isActive = (path: string) =>
  path === '/' ? route.path === '/' : route.path === path || route.path.startsWith(path + '/')
