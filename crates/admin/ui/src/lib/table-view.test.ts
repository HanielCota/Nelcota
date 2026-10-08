import { describe, expect, it } from 'vitest'
import { parseTableView, tableViewSearch } from './table-view'
import { parseFilters } from './filters'
import { rowKey, rowPk } from './grid'

describe('table context', () => {
  it('preserves filters while changing page, size and sorting', () => {
    const search = tableViewSearch(new URLSearchParams('status=eq.paid'), { page: 2, size: '100', sort: { column: 'created_at', desc: true } })
    const params = new URLSearchParams(search)
    expect(parseFilters(params)).toEqual([{ column: 'status', op: 'eq', value: 'paid' }])
    expect(parseTableView(params)).toEqual({ page: 2, size: '100', sort: { column: 'created_at', desc: true } })
    expect(parseTableView(new URLSearchParams(tableViewSearch(params, { page: 0, sort: null })))).toEqual({ page: 0, size: '100', sort: null })
  })

  it('rejects malformed paging parameters without affecting valid filters', () => {
    const query = new URLSearchParams('_page=-1&_size=999999&title=ilike.*test*')
    expect(parseTableView(query)).toEqual({ page: 0, size: '50', sort: null })
    expect(parseFilters(query)).toHaveLength(1)
  })
})

describe('row identity', () => {
  it('keeps the original composite primary key after reordering or editing', () => {
    const rows = [{ tenant: 'a', id: '1', title: 'first' }, { tenant: 'b', id: '1', title: 'second' }]
    const pk = rowPk(rows[0], ['tenant', 'id'])
    const key = rowKey(pk, ['tenant', 'id'])
    rows.reverse()
    rows[1].id = '2'
    expect(pk).toEqual({ tenant: 'a', id: '1' })
    expect(key).not.toBe(rowKey(rows[0], ['tenant', 'id']))
    expect(key).not.toBe(rowKey(rows[1], ['tenant', 'id']))
  })
})
