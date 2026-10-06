import { describe, expect, it } from 'vitest'
import { toCsv, toJson } from './download'

describe('CSV', () => {
  it('tem BOM, CRLF e aspas só quando precisa', () => {
    const csv = toCsv(['id', 'nome'], [
      ['1', 'Ana'],
      ['2', 'Régua, 30cm'],
      ['3', 'diz "oi"'],
    ])
    expect(csv).toBe('﻿id,nome\r\n1,Ana\r\n2,"Régua, 30cm"\r\n3,"diz ""oi"""\r\n')
  })

  it('NULL vira campo vazio; texto vazio também fica vazio', () => {
    expect(toCsv(['a', 'b'], [[null, '']])).toBe('﻿a,b\r\n,\r\n')
  })

  it('preserva quebra de linha dentro do campo', () => {
    expect(toCsv(['t'], [['linha 1\nlinha 2']])).toContain('"linha 1\nlinha 2"')
  })
})

describe('JSON', () => {
  it('monta objetos por coluna, mantendo o texto do Postgres', () => {
    const parsed = JSON.parse(toJson(['id', 'preco'], [['1', '2.50'], ['2', null]]))
    expect(parsed).toEqual([
      { id: '1', preco: '2.50' },
      { id: '2', preco: null },
    ])
  })
})
