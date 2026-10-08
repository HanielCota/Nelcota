export interface TableView {
  page: number
  size: string
  sort: { column: string; desc: boolean } | null
}

const SIZES = ['25', '50', '100', '500']

/** UI parameters are distinct from REST-style column filters. */
export function parseTableView(query: URLSearchParams): TableView {
  const rawPage = Number(query.get('_page') ?? 0)
  const size = query.get('_size') ?? '50'
  const column = query.get('_sort')
  return {
    page: Number.isSafeInteger(rawPage) && rawPage >= 0 ? rawPage : 0,
    size: SIZES.includes(size) ? size : '50',
    sort: column ? { column, desc: query.get('_desc') === 'true' } : null,
  }
}

export function tableViewSearch(query: URLSearchParams, patch: Partial<TableView>): string {
  const params = new URLSearchParams(query)
  if (patch.page !== undefined) patch.page ? params.set('_page', String(patch.page)) : params.delete('_page')
  if (patch.size !== undefined) patch.size === '50' ? params.delete('_size') : params.set('_size', patch.size)
  if (patch.sort !== undefined) {
    params.delete('_sort')
    params.delete('_desc')
    if (patch.sort) {
      params.set('_sort', patch.sort.column)
      if (patch.sort.desc) params.set('_desc', 'true')
    }
  }
  return params.toString()
}
