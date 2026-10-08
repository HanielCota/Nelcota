// SQL preview while a form changes. The server is the only source of the
// generated SQL (the same functions that execute it): the interface does not
// reimplement any rule. Rapid requests are debounced and the previous one is
// cancelled.

import { isAbort } from '$lib/api'
import { errorMessage } from '$lib/i18n/index.svelte'
import type { DdlResult } from './ddl'

const DELAY_MS = 350

export class SqlPreview {
  sql = $state<string[] | null>(null)
  error = $state('')
  loading = $state(false)

  #timer: ReturnType<typeof setTimeout> | undefined
  #controller: AbortController | undefined

  /** Schedules the preview; `request` gets the signal so it can be cancelled. */
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
      this.error = errorMessage(e)
    } finally {
      if (this.#controller === controller) this.loading = false
    }
  }

  /** Clears (e.g. an incomplete form) and cancels anything pending. */
  clear() {
    clearTimeout(this.#timer)
    this.#controller?.abort()
    this.sql = null
    this.error = ''
    this.loading = false
  }
}
