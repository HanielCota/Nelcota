// Filtros da grade de tabelas.
//
// Na URL do painel ficam no formato da API REST (`?preco=gte.10&nome=ilike.*a*`),
// então dá para compartilhar o link e voltar com o botão do navegador. Para a
// API do painel vão como JSON tipado (`filters=[...]`), validado no servidor.

export type FilterOp = 'eq' | 'neq' | 'gt' | 'gte' | 'lt' | 'lte' | 'ilike' | 'is'

export interface TableFilter {
  column: string
  op: FilterOp
  value: string
  not?: boolean
}

const OPS: readonly FilterOp[] = ['eq', 'neq', 'gt', 'gte', 'lt', 'lte', 'ilike', 'is']
const isOp = (op: string): op is FilterOp => (OPS as readonly string[]).includes(op)

/** Operadores como aparecem no formulário (alguns viram dois campos no fio). */
export type UiOp = 'eq' | 'neq' | 'gt' | 'gte' | 'lt' | 'lte' | 'contains' | 'null' | 'notnull'

export const UI_OPERATORS: readonly { value: UiOp; label: string; needsValue: boolean }[] = [
  { value: 'eq', label: 'igual a', needsValue: true },
  { value: 'neq', label: 'diferente de', needsValue: true },
  { value: 'contains', label: 'contém', needsValue: true },
  { value: 'gt', label: 'maior que', needsValue: true },
  { value: 'gte', label: 'maior ou igual a', needsValue: true },
  { value: 'lt', label: 'menor que', needsValue: true },
  { value: 'lte', label: 'menor ou igual a', needsValue: true },
  { value: 'null', label: 'é NULL', needsValue: false },
  { value: 'notnull', label: 'não é NULL', needsValue: false },
]

export const needsValue = (op: UiOp) => UI_OPERATORS.find((o) => o.value === op)?.needsValue ?? true

export function fromUi(column: string, op: UiOp, value: string): TableFilter {
  switch (op) {
    case 'contains':
      return { column, op: 'ilike', value: `*${value}*` }
    case 'null':
      return { column, op: 'is', value: 'null' }
    case 'notnull':
      return { column, op: 'is', value: 'null', not: true }
    default:
      return { column, op, value }
  }
}

export function toUi(filter: TableFilter): { op: UiOp; value: string } {
  if (filter.op === 'is' && filter.value === 'null') return { op: filter.not ? 'notnull' : 'null', value: '' }
  const contains = filter.op === 'ilike' && /^\*.*\*$/.test(filter.value)
  if (contains && !filter.not) return { op: 'contains', value: filter.value.slice(1, -1) }
  return { op: filter.op as UiOp, value: filter.value }
}

/** Texto curto para o chip do filtro: `preco ≥ 10`, `nome contém a`. */
export function describe(filter: TableFilter): string {
  const { op, value } = toUi(filter)
  const label = UI_OPERATORS.find((o) => o.value === op)?.label ?? filter.op
  const prefix = filter.not && op !== 'notnull' ? 'não ' : ''
  return needsValue(op) ? `${filter.column} ${prefix}${label} ${value}` : `${filter.column} ${label}`
}

/** Lê os filtros da query string da URL; pares inválidos são ignorados. */
export function parseFilters(query: URLSearchParams): TableFilter[] {
  const filters: TableFilter[] = []
  for (const [column, raw] of query) {
    const not = raw.startsWith('not.')
    const rest = not ? raw.slice(4) : raw
    const dot = rest.indexOf('.')
    if (dot < 0) continue
    const op = rest.slice(0, dot)
    if (!isOp(op)) continue
    filters.push({ column, op, value: rest.slice(dot + 1), ...(not && { not }) })
  }
  return filters
}

/** Inverso de `parseFilters`: `preco=gte.10&nome=not.is.null`. */
export function filtersToSearch(filters: readonly TableFilter[]): string {
  const query = new URLSearchParams()
  for (const f of filters) query.append(f.column, `${f.not ? 'not.' : ''}${f.op}.${f.value}`)
  return query.toString()
}

/** Formato da API do painel: JSON, ou `undefined` sem filtros. */
export function filtersParam(filters: readonly TableFilter[]): string | undefined {
  return filters.length ? JSON.stringify(filters) : undefined
}
