// Estado do editor SQL: rascunho, histórico e consultas salvas. Fica fora do
// componente para a paleta de comandos poder abrir uma consulta no editor.
// Persistido no navegador (por projeto, já que cada painel tem a sua origem).

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

// Formato versionado: `{ v: 1, items }`, para poder migrar no futuro.
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
  /** Consulta salva aberta no editor (`null` = rascunho solto). */
  currentId = $state<string | null>(null)

  current = $derived(this.saved.find((q) => q.id === this.currentId) ?? null)
  /** Texto do editor difere do que está salvo. */
  dirty = $derived(this.current !== null && this.current.sql !== this.draft)
  /** Mais recentes primeiro. */
  sorted = $derived([...this.saved].sort((a, b) => b.updatedAt - a.updatedAt))

  setDraft(sql: string) {
    this.draft = sql
    write(DRAFT_KEY, sql)
  }

  /** Abre um texto no editor; com `id`, passa a editar aquela consulta salva. */
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

  /** Atualiza a consulta aberta ou cria uma nova (`asNew`). */
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
