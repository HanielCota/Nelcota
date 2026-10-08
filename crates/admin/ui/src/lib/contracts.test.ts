import { expect, it } from 'vitest'
import { responseContract } from './contracts'

it('rejects incompatible rows and SQL results rather than trusting the client generic', () => {
  const sql = responseContract('POST', '/sql')!
  expect(sql({ results: [{ columns: ['id'], rows: [['9223372036854775807', null]], count: 1, truncated: false }], results_truncated: false })).toBe(true)
  expect(sql({ results: [{ columns: ['id'], rows: [[42]], count: 1, truncated: false }], results_truncated: false })).toBe(false)
  expect(sql({ error: { message: 'failed', code: null, hint: null, detail: null, position: null } })).toBe(true)
  expect(sql({ unexpected: 'result' })).toBe(false)
})
it('requires commit status and distinguishes preview from a committed change pending catalog refresh', () => {
  const ddl = responseContract('PATCH', '/tables/notes')!
  expect(ddl({ sql: ['ALTER TABLE notes ENABLE ROW LEVEL SECURITY'], applied: true, catalog_pending: true })).toBe(true)
  expect(ddl({ sql: ['SELECT 1'], applied: false, catalog_pending: false })).toBe(true)
  expect(ddl({ sql: ['SELECT 1'] })).toBe(false)
  expect(responseContract('POST', '/tables/notes/rows')).toBeUndefined()
})
it('requires enabled storage to carry its complete configuration', () => {
  const storage = responseContract('GET', '/storage')!
  expect(storage({ enabled: false })).toBe(true)
  expect(storage({ enabled: true })).toBe(false)
})
