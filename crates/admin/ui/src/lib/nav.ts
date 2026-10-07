import type { Component } from 'svelte'
import House from '@lucide/svelte/icons/house'
import Table2 from '@lucide/svelte/icons/table-2'
import SquareTerminal from '@lucide/svelte/icons/square-terminal'
import History from '@lucide/svelte/icons/history'
import Users from '@lucide/svelte/icons/users'
import ShieldCheck from '@lucide/svelte/icons/shield-check'
import Boxes from '@lucide/svelte/icons/boxes'
import Plug from '@lucide/svelte/icons/plug'
import type { MessageKey } from './i18n/index.svelte'
import { route } from './router.svelte'

/** Titles are translation keys: render them with `t(item.title)`. */
export type NavItem = { title: MessageKey; path: string; icon: Component }
export type NavGroup = { label?: MessageKey; items: NavItem[] }

// Sidebar groups; the first one has no heading.
export const navGroups: NavGroup[] = [
  { items: [{ title: 'shell.pages.overview', path: '/', icon: House }] },
  {
    label: 'shell.groups.database',
    items: [
      { title: 'shell.pages.tables', path: '/tables', icon: Table2 },
      { title: 'shell.pages.sql', path: '/sql', icon: SquareTerminal },
      { title: 'shell.pages.migrations', path: '/migrations', icon: History },
    ],
  },
  {
    label: 'shell.groups.access',
    items: [
      { title: 'shell.pages.users', path: '/users', icon: Users },
      { title: 'shell.pages.policies', path: '/policies', icon: ShieldCheck },
    ],
  },
  {
    label: 'shell.groups.integration',
    // /connect, not /api: /admin/api/* is the panel API prefix.
    items: [{ title: 'shell.pages.connect', path: '/connect', icon: Plug }],
  },
]

// Projects stays out of the menu: the project switcher at the top of the sidebar leads there.
export const navItems: NavItem[] = [
  ...navGroups.flatMap((group) => group.items),
  { title: 'shell.pages.projects', path: '/projects', icon: Boxes },
]

export const isActive = (path: string) =>
  path === '/' ? route.path === '/' : route.path === path || route.path.startsWith(path + '/')
