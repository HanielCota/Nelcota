import type { Component } from 'svelte'
import House from '@lucide/svelte/icons/house'
import Table2 from '@lucide/svelte/icons/table-2'
import SquareTerminal from '@lucide/svelte/icons/square-terminal'
import History from '@lucide/svelte/icons/history'
import Users from '@lucide/svelte/icons/users'
import ShieldCheck from '@lucide/svelte/icons/shield-check'
import Boxes from '@lucide/svelte/icons/boxes'
import Plug from '@lucide/svelte/icons/plug'
import HardDrive from '@lucide/svelte/icons/hard-drive'
import LogIn from '@lucide/svelte/icons/log-in'
import type { MessageKey } from '$lib/i18n/index.svelte'
import { route } from '$lib/router.svelte'

/** Titles are translation keys: render them with `t(item.title)`. */
export type NavItem = {
  title: MessageKey
  path: string
  icon: Component
  /** Other pages this pill stays lit for (sub-pages reached from it). */
  also?: string[]
}

// The pills of the top navigation (D98), in reading order. Sign-in settings
// live under Users; /connect, not /api, since /admin/api/* is the panel API.
export const navPills: NavItem[] = [
  { title: 'shell.pages.overview', path: '/', icon: House },
  { title: 'shell.pages.tables', path: '/tables', icon: Table2 },
  { title: 'shell.pages.sql', path: '/sql', icon: SquareTerminal },
  { title: 'shell.pages.migrations', path: '/migrations', icon: History },
  { title: 'shell.pages.storage', path: '/storage', icon: HardDrive },
  { title: 'shell.pages.users', path: '/users', icon: Users, also: ['/sign-in'] },
  { title: 'shell.pages.policies', path: '/policies', icon: ShieldCheck },
  { title: 'shell.pages.connect', path: '/connect', icon: Plug },
]

// Every page the command palette can open: the pills, plus pages reached
// from inside them (sign-in from Users, projects from the project switcher).
export const navItems: NavItem[] = [
  ...navPills,
  { title: 'shell.pages.userSignIn', path: '/sign-in', icon: LogIn },
  { title: 'shell.pages.projects', path: '/projects', icon: Boxes },
]

const matches = (path: string) => (path === '/' ? route.path === '/' : route.path === path || route.path.startsWith(path + '/'))

export const isActive = (path: string, also: readonly string[] = []) => matches(path) || also.some(matches)
