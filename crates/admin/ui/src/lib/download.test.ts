import { describe, expect, it } from 'vitest'
import { toCsv, toJson } from './download'

describe('CSV', () => {
  it('has a BOM, CRLF and quotes only when needed', () => {
    const csv = toCsv(['id', 'name'], [
      ['1', 'Ana'],
      ['2', 'Ruler, 30cm'],
      ['3', 'says "hi"'],
    ])
    expect(csv).toBe('﻿id,name\r\n1,Ana\r\n2,"Ruler, 30cm"\r\n3,"says ""hi"""\r\n')
  })

  it('NULL becomes an empty field; empty text stays empty too', () => {
    expect(toCsv(['a', 'b'], [[null, '']])).toBe('﻿a,b\r\n,\r\n')
  })

  it('keeps line breaks inside the field', () => {
    expect(toCsv(['t'], [['line 1\nline 2']])).toContain('"line 1\nline 2"')
  })
})

describe('JSON', () => {
  it('builds objects per column, keeping the Postgres text', () => {
    const parsed = JSON.parse(toJson(['id', 'price'], [['1', '2.50'], ['2', null]]))
    expect(parsed).toEqual([
      { id: '1', price: '2.50' },
      { id: '2', price: null },
    ])
  })
})
