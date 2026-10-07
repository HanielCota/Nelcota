import { describe as group, expect, it } from 'vitest'
import { describe, filtersParam, filtersToSearch, fromUi, parseFilters, toUi, type TableFilter } from './filters'

group('filtros na URL', () => {
  it('lê o formato da API REST, inclusive negação e valores com ponto', () => {
    const query = new URLSearchParams('preco=gte.10&nome=ilike.*a.b*&estoque=not.eq.0')
    expect(parseFilters(query)).toEqual([
      { column: 'preco', op: 'gte', value: '10' },
      { column: 'nome', op: 'ilike', value: '*a.b*' },
      { column: 'estoque', op: 'eq', value: '0', not: true },
    ])
  })

  it('ignora pares que não são filtros', () => {
    expect(parseFilters(new URLSearchParams('x=semponto&y=in.(1,2)&z=drop.table'))).toEqual([])
  })

  it('ida e volta preserva os filtros, inclusive acentos e &', () => {
    const filters: TableFilter[] = [
      { column: 'descrição', op: 'eq', value: 'a&b=c' },
      { column: 'id', op: 'is', value: 'null', not: true },
    ]
    expect(parseFilters(new URLSearchParams(filtersToSearch(filters)))).toEqual(filters)
  })

  it('aceita mais de um filtro na mesma coluna', () => {
    const query = new URLSearchParams('preco=gt.1&preco=lt.10')
    expect(parseFilters(query).map((f) => f.op)).toEqual(['gt', 'lt'])
  })
})

group('operadores do formulário', () => {
  it('"contém" vira ilike com curingas e volta', () => {
    const wire = fromUi('nome', 'contains', 'can')
    expect(wire).toEqual({ column: 'nome', op: 'ilike', value: '*can*' })
    expect(toUi(wire)).toEqual({ op: 'contains', value: 'can' })
  })

  it('NULL e não NULL usam is (com e sem not)', () => {
    expect(fromUi('x', 'null', '')).toEqual({ column: 'x', op: 'is', value: 'null' })
    expect(toUi(fromUi('x', 'notnull', ''))).toEqual({ op: 'notnull', value: '' })
  })

  it('descreve o filtro de forma legível', () => {
    expect(describe(fromUi('preco', 'gte', '10'))).toBe('preco maior ou igual a 10')
    expect(describe(fromUi('email', 'notnull', ''))).toBe('email não é NULL')
    expect(describe({ column: 'n', op: 'eq', value: '0', not: true })).toBe('n não igual a 0')
  })
})

group('parâmetro da API', () => {
  it('omite o parâmetro sem filtros e serializa como JSON com eles', () => {
    expect(filtersParam([])).toBeUndefined()
    expect(JSON.parse(filtersParam([{ column: 'a', op: 'eq', value: '1' }])!)).toEqual([
      { column: 'a', op: 'eq', value: '1' },
    ])
  })
})
