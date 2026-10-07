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
  const header = Math.max(column.name.length * 7.5 + (column.is_pk ? 64 : 48), typeLabel.length * 6.8 + 48, 96)
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
