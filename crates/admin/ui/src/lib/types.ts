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
  /** Foreign key from a column to another exposed table. */
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

/** `GET /admin/api/migrations` */
export type MigrationsData = {
  migrations: {
    version: number
    name: string
    applied_on: string | null
    /** `null` when the migrations/ folder is not reachable by the server. */
    in_folder: boolean | null
    from_panel: boolean
  }[]
  pending: {
    id: number
    applied_at: string
    /** English text; older rows may hold the former Portuguese one. */
    summary: string
    statements: string[]
    /** Kind of change and its table/policy name, for a description in the panel language. */
    kind: string | null
    target: string | null
  }[]
  next_version: number
  folder: string | null
}

/** `POST /admin/api/migrations` */
export type ExportedMigration = { version: number; filename: string; sql: string; message: string }

/** `GET /admin/api/storage` */
export type StorageOverview =
  | { enabled: false }
  | {
      enabled: true
      backend: 'disk' | 's3'
      max_file_size: number
      max_total_size: number | null
      /** Origin of public file URLs, when it is not the API's own. */
      public_url: string | null
      buckets: Bucket[]
    }

export type Bucket = {
  id: string
  public: boolean
  file_size_limit: number | null
  allowed_mime_types: string[] | null
  created_at: string
  files: number
  bytes: number
}

export type StoredFile = {
  id: string
  /** Full path inside the bucket. */
  name: string
  size: number
  mime_type: string
  owner: string | null
  updated_at: string
}

/** `GET /admin/api/storage/buckets/{id}/objects` */
export type StorageListing = { folders: string[]; objects: StoredFile[]; has_next: boolean }
