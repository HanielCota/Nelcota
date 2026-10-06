export type RlsState = 'ok' | 'warn' | 'danger' | 'none' | 'view'

export interface Rls {
  state: RlsState
  label: string
  enabled: boolean
  forced: boolean
  policies: number
}

export interface TableSummary {
  name: string
  kind: string
  has_pk: boolean
  rls: Rls
}

export interface Overview {
  schema: string
  counts: { tables: number; users: number; policies: number; functions: number }
  exposed_without_rls: string[]
  tables: {
    name: string
    kind: string
    comment: string | null
    rows: number | null
    rows_exact: boolean
    rls: Rls
    grants: { anon: string[]; authenticated: string[] }
  }[]
}

export interface Column {
  name: string
  type: string
  full_type: string
  category: string
  nullable: boolean
  has_default: boolean
  generated: boolean
  enum_values: string[]
  is_pk: boolean
  comment: string | null
  /** Chave estrangeira de uma coluna para outra tabela exposta. */
  references: { table: string; column: string } | null
}

export type RowData = Record<string, string | null>

export interface TableData {
  table: {
    name: string
    kind: string
    comment: string | null
    primary_key: string[]
    editable: boolean
    insertable: boolean
    exposed_without_rls: boolean
    rls: Rls
    columns: Column[]
  }
  rows: RowData[]
  page: number
  size: number
  has_next: boolean
  total: number | null
  total_exact: boolean
}

export interface User {
  id: string
  email: string
  created_at: string
  last_sign_in_at: string | null
  email_confirmed_at: string | null
  sessions: number
}

export interface Policy {
  name: string
  permissive: boolean
  roles: string[]
  command: string
  using: string | null
  check: string | null
}

export interface PoliciesData {
  schema: string
  exposed_without_rls: string[]
  tables: { name: string; rls: Rls; exposed_without_rls: boolean; policies: Policy[] }[]
  anon_functions: string[]
}

export interface SqlResult {
  columns: string[]
  rows: (string | null)[][]
  count: number
  truncated: boolean
}

export interface SqlResponse {
  results?: SqlResult[]
  error?: { message: string; code?: string; detail?: string; hint?: string; position?: number }
}

export interface ProjectLink {
  name: string
  url: string | null
  current: boolean
}

export interface ProjectsData {
  current: string
  sso: boolean
  projects: ProjectLink[]
}

export interface ProjectStatus extends ProjectLink {
  healthy: boolean
  version: string | null
  latency_ms: number | null
}
