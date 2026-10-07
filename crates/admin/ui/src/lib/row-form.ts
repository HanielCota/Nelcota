// Regras do formulário de linha (inserir/editar), separadas do componente
// para testar sem montar a interface.

import { columnKind } from './grid'
import type { Column, RowData } from './types'

export interface FieldState {
  value: string
  isNull: boolean
}

/** Estado inicial dos campos: valores da linha ou vazio (inserção). */
export function initialFields(columns: readonly Column[], row: RowData | null): Record<string, FieldState> {
  return Object.fromEntries(
    columns.map((c) => {
      const value = row ? row[c.name] : null
      return [c.name, { value: value ?? '', isNull: row ? value === null : false }]
    }),
  )
}

/** Problema que impede salvar o campo, ou `null`. */
export function fieldProblem(column: Column, field: FieldState, inserting: boolean): string | null {
  if (field.isNull) return null
  if (columnKind(column) === 'json' && field.value.trim() !== '') {
    try {
      JSON.parse(field.value)
    } catch {
      return 'JSON inválido'
    }
  }
  // Na inserção, campo vazio vira DEFAULT; sem DEFAULT e sem NULL, é obrigatório.
  if (inserting && field.value === '' && !column.nullable && !column.has_default) return 'obrigatória'
  return null
}

/**
 * Valores a enviar. Edição: só o que difere da linha original (digitar e
 * voltar ao valor antigo não conta). Inserção: campo vazio fica de fora
 * (vale o DEFAULT), salvo NULL marcado.
 */
export function rowPayload(
  columns: readonly Column[],
  fields: Record<string, FieldState>,
  original: RowData | null,
): Record<string, string | null> {
  const values: Record<string, string | null> = {}
  for (const c of columns) {
    if (c.generated) continue
    const f = fields[c.name]
    if (!f) continue
    const value = f.isNull ? null : f.value
    if (original) {
      if (value !== original[c.name]) values[c.name] = value
    } else if (f.isNull) {
      values[c.name] = null
    } else if (f.value !== '') {
      values[c.name] = f.value
    }
  }
  return values
}
