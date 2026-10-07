// Prévia do SQL enquanto o formulário muda. O servidor é a única fonte do
// SQL gerado (as mesmas funções que executam): a interface não reimplementa
// regra nenhuma. Pedidos em sequência rápida são agrupados (debounce) e o
// anterior é cancelado.

import { isAbort } from './api'
import type { DdlResult } from './ddl'

const DELAY_MS = 350

export class SqlPreview {
  sql = $state<string[] | null>(null)
  error = $state('')
  loading = $state(false)

  #timer: ReturnType<typeof setTimeout> | undefined
  #controller: AbortController | undefined

  /** Agenda a prévia; `request` recebe o sinal para poder ser cancelado. */
  schedule(request: (signal: AbortSignal) => Promise<DdlResult>) {
    clearTimeout(this.#timer)
    this.#timer = setTimeout(() => this.#run(request), DELAY_MS)
  }

  async #run(request: (signal: AbortSignal) => Promise<DdlResult>) {
    this.#controller?.abort()
    const controller = (this.#controller = new AbortController())
    this.loading = true
    try {
      this.sql = (await request(controller.signal)).sql
      this.error = ''
    } catch (e) {
      if (isAbort(e)) return
      this.sql = null
      this.error = (e as Error).message
    } finally {
      if (this.#controller === controller) this.loading = false
    }
  }

  /** Limpa (ex.: formulário incompleto) e cancela o que estiver pendente. */
  clear() {
    clearTimeout(this.#timer)
    this.#controller?.abort()
    this.sql = null
    this.error = ''
    this.loading = false
  }
}
