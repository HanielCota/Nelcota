import { api } from '$lib/api'
import type { SqlResponse } from '$lib/types'
import type { SqlAdapter, SqlSchema } from './execution.svelte'

export const sqlAdapter: SqlAdapter = {
  schema: signal => api.get<SqlSchema>('/schema', { signal }),
  execute: (sql, signal) => api.post<SqlResponse>('/sql', { sql }, { signal }),
}
