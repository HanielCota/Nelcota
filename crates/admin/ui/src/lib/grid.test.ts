import { describe, expect, it } from 'vitest'
import { alignRight, columnKind, columnWidth, monospace } from './grid'

const col = (
  name: string,
  type: string,
  extra: { enum_values?: string[]; is_pk?: boolean; full_type?: string; references?: { table: string; column: string } } = {},
) => ({
  name,
  type,
  full_type: extra.full_type ?? type,
  references: extra.references ?? null,
  enum_values: extra.enum_values ?? [],
  is_pk: extra.is_pk ?? false,
})

describe('categoria da coluna', () => {
  it('pelo tipo do Postgres (nome do format_type)', () => {
    expect(columnKind(col('a', 'numeric'))).toBe('number')
    expect(columnKind(col('a', 'bigint'))).toBe('number')
    expect(columnKind(col('a', 'boolean'))).toBe('boolean')
    expect(columnKind(col('a', 'timestamp with time zone'))).toBe('temporal')
    expect(columnKind(col('a', 'jsonb'))).toBe('json')
    expect(columnKind(col('a', 'uuid'))).toBe('uuid')
    expect(columnKind(col('a', 'text'))).toBe('text')
    expect(columnKind(col('a', 'character varying'))).toBe('text')
  })

  it('enum vem antes do tipo', () => {
    expect(columnKind(col('a', 'status_pedido', { enum_values: ['aberto', 'pago'] }))).toBe('enum')
  })
})

describe('largura e alinhamento', () => {
  it('largura por tipo, sem cortar o nome no cabeçalho', () => {
    expect(columnWidth(col('id', 'bigint', { is_pk: true }))).toBe(96)
    expect(columnWidth(col('feito', 'boolean'))).toBe(104)
    expect(columnWidth(col('um_nome_de_coluna_bem_comprido', 'boolean'))).toBeGreaterThan(104)
    expect(columnWidth(col('x'.repeat(80), 'text'))).toBe(420)
  })

  it('a linha do tipo também cabe (tipo completo e tabela da chave estrangeira)', () => {
    const fk = col('cliente_id', 'bigint', { references: { table: 'clientes', column: 'id' } })
    expect(columnWidth(fk)).toBeGreaterThanOrEqual(Math.ceil('bigint → clientes'.length * 6.8 + 48))
    const numeric = col('total', 'numeric', { full_type: 'numeric(10,2)' })
    expect(columnWidth(numeric)).toBeGreaterThanOrEqual(Math.ceil('numeric(10,2)'.length * 6.8 + 48))
    expect(columnWidth(col('criado_em', 'timestamp with time zone'))).toBeGreaterThanOrEqual(188)
  })

  it('números à direita e em mono; texto em fonte normal', () => {
    expect(alignRight('number')).toBe(true)
    expect(alignRight('text')).toBe(false)
    expect(monospace('number')).toBe(true)
    expect(monospace('text')).toBe(false)
    expect(monospace('enum')).toBe(false)
  })
})
