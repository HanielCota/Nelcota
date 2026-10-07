// Colunas ocultas da grade, lembradas por tabela neste navegador.

import { readJson, write } from './storage'

const key = (table: string) => `nelcota:hidden-columns:${table}`
const parse = (data: unknown): string[] | null =>
  Array.isArray(data) ? data.filter((c): c is string => typeof c === 'string') : null

export class HiddenColumns {
  names = $state<string[]>([])
  #table = ''

  /** Troca para as preferências de outra tabela. */
  load(table: string) {
    this.#table = table
    this.names = readJson(key(table), parse, [])
  }

  has(column: string) {
    return this.names.includes(column)
  }

  hide(column: string) {
    if (!this.has(column)) this.set([...this.names, column])
  }

  show(column: string) {
    this.set(this.names.filter((c) => c !== column))
  }

  toggle(column: string) {
    if (this.has(column)) this.show(column)
    else this.hide(column)
  }

  showAll() {
    this.set([])
  }

  /** Esquece colunas que não existem mais (renomeadas ou apagadas). */
  prune(existing: readonly string[]) {
    const kept = this.names.filter((c) => existing.includes(c))
    if (kept.length !== this.names.length) this.set(kept)
  }

  private set(names: string[]) {
    this.names = names
    if (this.#table) write(key(this.#table), JSON.stringify(names))
  }
}
