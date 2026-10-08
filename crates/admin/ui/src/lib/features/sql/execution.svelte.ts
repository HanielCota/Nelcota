import { RemoteResource } from '$lib/remote-resource.svelte'
import type { SqlResponse } from '$lib/types'

export type SqlSchema = { schema: string; tables: Record<string, string[]> }
export interface SqlAdapter {
  schema(signal: AbortSignal): Promise<SqlSchema>
  execute(sql: string, signal: AbortSignal): Promise<SqlResponse>
}

/** Owns SQL request lifetime, timing and results independently of the page. */
export class SqlExecution {
  readonly schema = new RemoteResource<SqlSchema>()
  running = $state(false)
  response = $state<SqlResponse | null>(null)
  error = $state<unknown>(null)
  elapsed = $state(0)
  private generation = 0
  private controller?: AbortController

  constructor(private adapter: SqlAdapter, private now = () => performance.now()) {}

  loadSchema() { return this.schema.load(signal => this.adapter.schema(signal)) }

  async run(sql: string): Promise<boolean> {
    if (this.running || !sql.trim()) return false
    const generation = ++this.generation
    const controller = this.controller = new AbortController()
    const started = this.now()
    this.running = true
    this.error = null
    try {
      const response = await this.adapter.execute(sql, controller.signal)
      if (generation !== this.generation) return false
      this.response = response
      return true
    } catch (error) {
      if (generation === this.generation && !controller.signal.aborted) this.error = error
      return false
    } finally {
      if (generation === this.generation) {
        this.elapsed = Math.round(this.now() - started)
        this.running = false
        this.controller = undefined
      }
    }
  }

  cancel() {
    this.generation++
    this.controller?.abort()
    this.controller = undefined
    this.running = false
    this.schema.cancel()
  }
}
