import { describe, expect, it } from 'vitest'
import { fieldProblem, initialFields, rowPayload } from './row-form'
import type { Column } from './types'

const col = (name: string, type: string, extra: Partial<Column> = {}): Column => ({
  name,
  type,
  full_type: type,
  category: 'S',
  nullable: true,
  has_default: false,
  generated: false,
  enum_values: [],
  is_pk: false,
  comment: null,
  references: null,
  ...extra,
})

const columns = [
  col('id', 'bigint', { generated: true, is_pk: true, nullable: false, has_default: true }),
  col('nome', 'text', { nullable: false }),
  col('extra', 'jsonb'),
  col('criado_em', 'timestamp with time zone', { nullable: false, has_default: true }),
]
const row = { id: '1', nome: 'Ana', extra: null, criado_em: '2026-10-06T22:26:15+00:00' }

describe('campos iniciais', () => {
  it('da linha, com NULL marcado; vazios na inserção', () => {
    expect(initialFields(columns, row).extra).toEqual({ value: '', isNull: true })
    expect(initialFields(columns, null).nome).toEqual({ value: '', isNull: false })
  })
})

describe('validação', () => {
  it('JSON inválido', () => {
    expect(fieldProblem(columns[2], { value: '{a:1}', isNull: false }, false)).toBe('JSON inválido')
    expect(fieldProblem(columns[2], { value: '{"a":1}', isNull: false }, false)).toBeNull()
  })

  it('obrigatória só na inserção, sem DEFAULT e sem NULL', () => {
    expect(fieldProblem(columns[1], { value: '', isNull: false }, true)).toBe('obrigatória')
    expect(fieldProblem(columns[3], { value: '', isNull: false }, true)).toBeNull()
    expect(fieldProblem(columns[1], { value: '', isNull: false }, false)).toBeNull()
  })
})

describe('o que vai para o servidor', () => {
  it('edição: só o que mudou de verdade', () => {
    const fields = initialFields(columns, row)
    fields.nome = { value: 'Ana', isNull: false } // digitou e voltou ao original
    fields.extra = { value: '{"vip":true}', isNull: false }
    expect(rowPayload(columns, fields, row)).toEqual({ extra: '{"vip":true}' })
  })

  it('edição: marcar NULL envia null', () => {
    const fields = initialFields(columns, row)
    fields.nome = { value: 'Ana', isNull: true }
    expect(rowPayload(columns, fields, row)).toEqual({ nome: null })
  })

  it('inserção: vazio fica de fora (DEFAULT), NULL explícito vai, gerada nunca', () => {
    const fields = initialFields(columns, null)
    fields.nome = { value: 'Bruno', isNull: false }
    fields.extra = { value: '', isNull: true }
    fields.id = { value: '99', isNull: false }
    expect(rowPayload(columns, fields, null)).toEqual({ nome: 'Bruno', extra: null })
  })
})
