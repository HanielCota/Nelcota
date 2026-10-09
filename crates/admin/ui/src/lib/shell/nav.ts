import type { Component } from 'svelte'
import House from '@lucide/svelte/icons/house'
import Rows3 from '@lucide/svelte/icons/rows-3'
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
  /** Short name on the top pill (defaults to the title). */
  pill?: MessageKey
  path: string
  icon: Component
  /** Other pages this pill stays lit for (sub-pages reached from it). */
  also?: string[]
}

// The pills of the top navigation (D98), in reading order. Sign-in settings
// live under Users; /connect, not /api, since /admin/api/* is the panel API.
export const navPills: NavItem[] = [
  { title: 'shell.pages.overview', pill: 'shell.pills.overview', path: '/', icon: House },
  { title: 'shell.pages.tables', pill: 'shell.pills.tables', path: '/tables', icon: Rows3 },
  { title: 'shell.pages.sql', pill: 'shell.pills.sql', path: '/sql', icon: SquareTerminal },
  { title: 'shell.pages.migrations', pill: 'shell.pills.migrations', path: '/migrations', icon: History },
  { title: 'shell.pages.storage', pill: 'shell.pills.storage', path: '/storage', icon: HardDrive },
  { title: 'shell.pages.users', pill: 'shell.pills.users', path: '/users', icon: Users, also: ['/sign-in'] },
  { title: 'shell.pages.policies', pill: 'shell.pills.policies', path: '/policies', icon: ShieldCheck },
  { title: 'shell.pages.connect', pill: 'shell.pills.connect', path: '/connect', icon: Plug },
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
