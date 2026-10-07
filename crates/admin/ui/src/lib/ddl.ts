// Table and policy structure: types mirroring the panel API, the client and
// the pure rules (difference between columns, fields of each policy).

import { api, enc } from './api'
import type { MessageKey } from './i18n/index.svelte'

export type ApiRole = 'anon' | 'authenticated' | 'service_role'
export type Privilege = 'select' | 'insert' | 'update' | 'delete'
export type OnDelete = 'no_action' | 'restrict' | 'cascade' | 'set_null' | 'set_default'

export const API_ROLES: readonly ApiRole[] = ['anon', 'authenticated', 'service_role']
export const PRIVILEGES: readonly Privilege[] = ['select', 'insert', 'update', 'delete']

/** `label` is a translation key: render it with `t(option.label)`. */
export const ON_DELETE: readonly { value: OnDelete; label: MessageKey }[] = [
  { value: 'no_action', label: 'tables.columns.onDelete.no_action' },
  { value: 'cascade', label: 'tables.columns.onDelete.cascade' },
  { value: 'set_null', label: 'tables.columns.onDelete.set_null' },
  { value: 'restrict', label: 'tables.columns.onDelete.restrict' },
  { value: 'set_default', label: 'tables.columns.onDelete.set_default' },
]

export interface ReferenceDef {
  table: string
  column: string
  on_delete: OnDelete
}

export interface ColumnDef {
  name: string
  data_type: string
  nullable: boolean
  default: string | null
  primary_key: boolean
  identity: boolean
  unique: boolean
  references: ReferenceDef | null
  comment: string | null
}

export interface GrantDef {
  role: ApiRole
  privileges: Privilege[]
}

export interface CreateTable {
  name: string
  comment: string | null
  columns: ColumnDef[]
  rls: boolean
  grants: GrantDef[]
}

export type AlterAction =
  | { action: 'rename_table'; name: string }
  | { action: 'set_comment'; comment: string | null }
  | { action: 'set_rls'; enabled: boolean }
  | { action: 'add_column'; column: ColumnDef }
  | { action: 'drop_column'; name: string }
  | { action: 'rename_column'; from: string; to: string }
  | { action: 'set_type'; column: string; data_type: string; using?: string | null }
  | { action: 'set_nullable'; column: string; nullable: boolean }
  | { action: 'set_default'; column: string; default: string | null }
  | { action: 'set_unique'; column: string; unique: boolean }
  | { action: 'set_reference'; column: string; reference: ReferenceDef | null }
  | { action: 'set_column_comment'; column: string; comment: string | null }
  | { action: 'set_grants'; grant: GrantDef }

// Structure read from pg_catalog (GET /tables/:name/structure).
export interface ColumnInfo {
  name: string
  data_type: string
  nullable: boolean
  default: string | null
  identity: 'always' | 'by default' | null
  generated: boolean
  primary_key: boolean
  unique: string | null
  references: { table: string; column: string; on_delete: string; constraint: string } | null
  comment: string | null
}

export interface Structure {
  name: string
  comment: string | null
  rls_enabled: boolean
  primary_key: string[]
  columns: ColumnInfo[]
  grants: { role: ApiRole; privileges: Privilege[] }[]
}

export type PolicyCommand = 'all' | 'select' | 'insert' | 'update' | 'delete'
export type PolicyRole = 'public' | ApiRole

export interface PolicyDef {
  name: string
  command: PolicyCommand
  roles: PolicyRole[]
  permissive: boolean
  using: string | null
  check: string | null
}

/** Response of every change: the SQL that ran (or its preview). */
export interface DdlResult {
  sql: string[]
  message?: string
}

export const blankColumn = (): ColumnDef => ({
  name: '',
  data_type: 'text',
  nullable: true,
  default: null,
  primary_key: false,
  identity: false,
  unique: false,
  references: null,
  comment: null,
})

const blank = (text: string | null | undefined) => (text?.trim() ? text.trim() : null)

/** `on_delete` as pg_catalog describes it (`set null`) in the API format. */
const onDeleteValue = (text: string): OnDelete => text.replace(' ', '_') as OnDelete

/** An existing column in the form's format. */
export function toColumnDef(info: ColumnInfo): ColumnDef {
  return {
    name: info.name,
    data_type: info.data_type,
    nullable: info.nullable,
    default: info.default,
    primary_key: info.primary_key,
    identity: info.identity !== null,
    unique: info.unique !== null,
    references: info.references
      ? { table: info.references.table, column: info.references.column, on_delete: onDeleteValue(info.references.on_delete) }
      : null,
    comment: info.comment,
  }
}

const sameReference = (a: ReferenceDef | null, b: ReferenceDef | null) =>
  a === b || (a !== null && b !== null && a.table === b.table && a.column === b.column && a.on_delete === b.on_delete)

/**
 * Actions that take column `original` to the `edited` state. The rename goes
 * last: the earlier actions use the old name (the server also follows renames,
 * but this way the preview reads in natural order).
 */
export function columnChanges(original: ColumnInfo, edited: ColumnDef): AlterAction[] {
  const column = original.name
  const before = toColumnDef(original)
  const actions: AlterAction[] = []
  if (edited.data_type.trim() !== before.data_type) {
    actions.push({ action: 'set_type', column, data_type: edited.data_type.trim() })
  }
  if (edited.nullable !== before.nullable) actions.push({ action: 'set_nullable', column, nullable: edited.nullable })
  if (blank(edited.default) !== blank(before.default) && original.identity === null && !original.generated) {
    actions.push({ action: 'set_default', column, default: blank(edited.default) })
  }
  if (edited.unique !== before.unique) actions.push({ action: 'set_unique', column, unique: edited.unique })
  if (!sameReference(edited.references, before.references)) {
    actions.push({ action: 'set_reference', column, reference: edited.references })
  }
  if (blank(edited.comment) !== blank(before.comment)) {
    actions.push({ action: 'set_column_comment', column, comment: blank(edited.comment) })
  }
  if (edited.name.trim() !== column) actions.push({ action: 'rename_column', from: column, to: edited.name.trim() })
  return actions
}

/** GRANTs that changed, as `set_grants` actions (one per changed role). */
export function grantChanges(before: Structure['grants'], after: GrantDef[]): AlterAction[] {
  return after
    .filter((grant) => {
      const old = before.find((b) => b.role === grant.role)?.privileges ?? []
      return old.length !== grant.privileges.length || old.some((p) => !grant.privileges.includes(p))
    })
    .map((grant) => ({ action: 'set_grants', grant }))
}

/** Which expressions each command accepts (same rule as the server). */
export function policyFields(command: PolicyCommand): { using: boolean; check: boolean } {
  return {
    using: command !== 'insert',
    check: command !== 'select' && command !== 'delete',
  }
}

/** A policy as GET /policies returns it, in the form's format. */
export function toPolicyDef(policy: {
  name: string
  command: string
  roles: string[]
  permissive: boolean
  using: string | null
  check: string | null
}): PolicyDef {
  const roles = policy.roles.filter((r): r is PolicyRole =>
    ['public', 'anon', 'authenticated', 'service_role'].includes(r),
  )
  return {
    name: policy.name,
    command: policy.command.toLowerCase() as PolicyCommand,
    roles: roles.includes('public') ? [] : roles,
    permissive: policy.permissive,
    using: policy.using,
    check: policy.check,
  }
}

type Options = { signal?: AbortSignal }

export const ddl = {
  types: () => api.get<{ base: string[]; enums: string[] }>('/types'),
  structure: (table: string) => api.get<Structure>(`/tables/${enc(table)}/structure`),
  createTable: (table: CreateTable, preview = false, options: Options = {}) =>
    api.post<DdlResult>('/tables', { table, preview }, options),
  alterTable: (table: string, actions: AlterAction[], preview = false, options: Options = {}) =>
    api.patch<DdlResult>(`/tables/${enc(table)}`, { actions, preview }, options),
  dropTable: (table: string, cascade: boolean) =>
    api.delete<DdlResult>(`/tables/${enc(table)}?cascade=${cascade}`),
  createPolicy: (table: string, policy: PolicyDef, preview = false, options: Options = {}) =>
    api.post<DdlResult>(`/tables/${enc(table)}/policies`, { policy, preview }, options),
  replacePolicy: (table: string, original: string, policy: PolicyDef, preview = false, options: Options = {}) =>
    api.put<DdlResult>(`/tables/${enc(table)}/policies/${enc(original)}`, { policy, preview }, options),
  dropPolicy: (table: string, policy: string) =>
    api.delete<DdlResult>(`/tables/${enc(table)}/policies/${enc(policy)}`),
}
