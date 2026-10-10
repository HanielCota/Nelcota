import { describe, expect, it } from 'vitest'
import { formatJson } from './json-format'

describe('JSON formatting', () => {
  it('preserves integer and decimal lexemes, exponents and escaped strings', () => {
    expect(formatJson('{"id":9007199254740993,"n":0.12345678901234567890,"e":1e400,"s":"a\\\"b\\u0063"}'))
      .toBe('{\n  "id": 9007199254740993,\n  "n": 0.12345678901234567890,\n  "e": 1e400,\n  "s": "a\\\"b\\u0063"\n}')
  })
  it('formats nested and empty containers without changing duplicate keys', () => {
    expect(formatJson('{"a":[{},[],true,null,{"x":1,"x":2}]}')).toBe(
      '{\n  "a": [\n    {},\n    [],\n    true,\n    null,\n    {\n      "x": 1,\n      "x": 2\n    }\n  ]\n}',
    )
  })
  it('rejects invalid JSON and accepts scalar JSON', () => {
    expect(() => formatJson('{a:1}')).toThrow()
    expect(formatJson(' 9007199254740993 ')).toBe('9007199254740993')
  })
})
