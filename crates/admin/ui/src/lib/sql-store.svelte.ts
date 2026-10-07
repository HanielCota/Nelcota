// SQL editor state: draft, history and saved queries. Lives outside the
// component so the command palette can open a query in the editor.
// Persisted in the browser (per project, since each panel has its own origin).

import { newId, readJson, readText, write } from './storage'

const DRAFT_KEY = 'nelcota:sql-draft'
const HISTORY_KEY = 'nelcota:sql-history'
const SAVED_KEY = 'nelcota:sql-saved'
const HISTORY_LIMIT = 20

export interface SavedQuery {
  id: string
  name: string
  sql: string
  updatedAt: number
}

const isSaved = (item: unknown): item is SavedQuery =>
  typeof item === 'object' &&
  item !== null &&
  typeof (item as SavedQuery).id === 'string' &&
  typeof (item as SavedQuery).name === 'string' &&
  typeof (item as SavedQuery).sql === 'string' &&
  typeof (item as SavedQuery).updatedAt === 'number'

// Versioned format: `{ v: 1, items }`, so it can be migrated later.
const parseSaved = (data: unknown): SavedQuery[] | null =>
  typeof data === 'object' && data !== null && (data as { v?: unknown }).v === 1
    ? ((data as { items?: unknown }).items as unknown[] | undefined)?.filter(isSaved) ?? []
    : null

const parseHistory = (data: unknown): string[] | null =>
  Array.isArray(data) ? data.filter((h): h is string => typeof h === 'string') : null

class SqlStore {
  draft = $state(readText(DRAFT_KEY, 'select now();'))
  history = $state<string[]>(readJson(HISTORY_KEY, parseHistory, []))
  saved = $state<SavedQuery[]>(readJson(SAVED_KEY, parseSaved, []))
  /** Saved query open in the editor (`null` = a loose draft). */
  currentId = $state<string | null>(null)

  current = $derived(this.saved.find((q) => q.id === this.currentId) ?? null)
  /** The editor text differs from what is saved. */
  dirty = $derived(this.current !== null && this.current.sql !== this.draft)
  /** Most recent first. */
  sorted = $derived([...this.saved].sort((a, b) => b.updatedAt - a.updatedAt))

  setDraft(sql: string) {
    this.draft = sql
    write(DRAFT_KEY, sql)
  }

  /** Opens a text in the editor; with `id`, starts editing that saved query. */
  open(sql: string, id: string | null = null) {
    this.setDraft(sql)
    this.currentId = id
  }

  openSaved(id: string) {
    const query = this.saved.find((q) => q.id === id)
    if (query) this.open(query.sql, query.id)
  }

  remember(sql: string) {
    this.history = [sql, ...this.history.filter((h) => h !== sql)].slice(0, HISTORY_LIMIT)
    write(HISTORY_KEY, JSON.stringify(this.history))
  }

  /** Updates the open query or creates a new one (`asNew`). */
  save(name: string, asNew = false): SavedQuery {
    const now = Date.now()
    const existing = asNew ? null : this.current
    const query: SavedQuery = existing
      ? { ...existing, name, sql: this.draft, updatedAt: now }
      : { id: newId(), name, sql: this.draft, updatedAt: now }
    this.saved = existing ? this.saved.map((q) => (q.id === query.id ? query : q)) : [...this.saved, query]
    this.currentId = query.id
    this.persist()
    return query
  }

  rename(id: string, name: string) {
    this.saved = this.saved.map((q) => (q.id === id ? { ...q, name, updatedAt: Date.now() } : q))
    this.persist()
  }

  remove(id: string) {
    this.saved = this.saved.filter((q) => q.id !== id)
    if (this.currentId === id) this.currentId = null
    this.persist()
  }

  private persist() {
    write(SAVED_KEY, JSON.stringify({ v: 1, items: this.saved }))
  }
}

export const sqlStore = new SqlStore()
