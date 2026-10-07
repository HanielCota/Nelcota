// Apresentação das colunas na grade conforme o tipo do Postgres: categoria,
// largura e alinhamento. Puro, para testar sem montar componente.

import type { Column } from './types'

export type ColumnKind = 'number' | 'boolean' | 'temporal' | 'json' | 'uuid' | 'enum' | 'text'

const NUMBER = ['smallint', 'integer', 'bigint', 'numeric', 'real', 'double precision']
const TEMPORAL = ['timestamp with time zone', 'timestamp without time zone', 'date', 'time without time zone', 'time with time zone', 'interval']

export function columnKind(column: Pick<Column, 'type' | 'enum_values'>): ColumnKind {
  const type = column.type
  if (column.enum_values.length) return 'enum'
  if (NUMBER.includes(type)) return 'number'
  if (type === 'boolean') return 'boolean'
  if (TEMPORAL.includes(type)) return 'temporal'
  if (type === 'json' || type === 'jsonb') return 'json'
  if (type === 'uuid') return 'uuid'
  return 'text'
}

/**
 * Largura inicial (px). Fixa por coluna: com `table-layout: fixed`, editar
 * uma célula não faz as outras colunas mudarem de tamanho.
 */
export function columnWidth(
  column: Pick<Column, 'name' | 'type' | 'enum_values' | 'is_pk' | 'full_type' | 'references'>,
): number {
  const kind = columnKind(column)
  // O cabeçalho precisa caber: nome (fonte normal) e linha do tipo (mono
  // pequena, com a tabela da chave estrangeira). Larguras médias por caractere.
  const typeLabel = column.full_type + (column.references ? ` → ${column.references.table}` : '')
  const header = Math.max(column.name.length * 7.5 + (column.is_pk ? 88 : 72), typeLabel.length * 6.8 + 72, 96)
  const base: Record<ColumnKind, number> = {
    number: column.is_pk ? 96 : 128,
    boolean: 104,
    temporal: 188,
    json: 260,
    uuid: 300,
    enum: 140,
    text: 220,
  }
  return Math.ceil(Math.min(Math.max(base[kind], header), 420))
}

/** Números à direita (comparar grandezas); o resto à esquerda. */
export const alignRight = (kind: ColumnKind) => kind === 'number'

/** Fonte mono só onde ajuda a ler (números, ids, JSON); texto em fonte normal. */
export const monospace = (kind: ColumnKind) => kind === 'number' || kind === 'uuid' || kind === 'json'

export interface PageInfo {
  /** Primeira e última linha mostradas (1-based); 0 e 0 sem linhas. */
  from: number
  to: number
  /** Última página (0-based), só com total exato; `null` se desconhecida. */
  lastPage: number | null
  pageCount: number | null
}

/** Faixa de linhas e total de páginas para o rodapé da grade. */
export function pageInfo(page: number, size: number, rowsOnPage: number, total: number | null, exact: boolean): PageInfo {
  const from = rowsOnPage ? page * size + 1 : 0
  const to = rowsOnPage ? page * size + rowsOnPage : 0
  if (total === null || !exact) return { from, to, lastPage: null, pageCount: null }
  const pageCount = Math.max(1, Math.ceil(total / size))
  return { from, to, lastPage: pageCount - 1, pageCount }
}

export interface CellPos {
  row: number
  col: number
}

const PAGE_ROWS = 10

/**
 * Navegação por teclado na grade (padrão "grid" do WAI-ARIA): setas, Home/End
 * (com Ctrl, primeira/última célula) e PageUp/PageDown. `null` para teclas que
 * não movem. Sempre dentro dos limites.
 */
export function nextCell(key: string, pos: CellPos, rows: number, cols: number, ctrl = false): CellPos | null {
  if (rows === 0 || cols === 0) return null
  const clamp = (row: number, col: number) => ({
    row: Math.min(Math.max(row, 0), rows - 1),
    col: Math.min(Math.max(col, 0), cols - 1),
  })
  switch (key) {
    case 'ArrowUp':
      return clamp(pos.row - 1, pos.col)
    case 'ArrowDown':
      return clamp(pos.row + 1, pos.col)
    case 'ArrowLeft':
      return clamp(pos.row, pos.col - 1)
    case 'ArrowRight':
      return clamp(pos.row, pos.col + 1)
    case 'Home':
      return ctrl ? clamp(0, 0) : clamp(pos.row, 0)
    case 'End':
      return ctrl ? clamp(rows - 1, cols - 1) : clamp(pos.row, cols - 1)
    case 'PageUp':
      return clamp(pos.row - PAGE_ROWS, pos.col)
    case 'PageDown':
      return clamp(pos.row + PAGE_ROWS, pos.col)
    default:
      return null
  }
}
