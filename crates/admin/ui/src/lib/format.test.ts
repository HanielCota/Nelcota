import { describe, expect, it } from 'vitest'
import { formatCell } from './format'

describe('datas na grade', () => {
  it('timestamptz vai para o fuso de quem vê, com o valor exato no tooltip', () => {
    const raw = '2026-10-06T22:26:15.177028+00:00'
    expect(formatCell(raw, 'timestamp with time zone', { timeZone: 'UTC' })).toEqual({
      text: '06/10/2026, 22:26:15',
      raw,
    })
    expect(formatCell(raw, 'timestamp with time zone', { timeZone: 'America/Sao_Paulo' }).text).toBe(
      '06/10/2026, 19:26:15',
    )
  })

  it('timestamp sem fuso fica como gravado (sem conversão)', () => {
    expect(formatCell('2026-10-06T22:26:15.5', 'timestamp without time zone').text).toBe('06/10/2026, 22:26:15')
    expect(formatCell('2026-01-02T03:04', 'timestamp without time zone').text).toBe('02/01/2026, 03:04:00')
  })

  it('date no formato brasileiro', () => {
    expect(formatCell('2026-10-06', 'date')).toEqual({ text: '06/10/2026', raw: '2026-10-06' })
  })

  it('valor desconhecido ou outro tipo sai como veio', () => {
    expect(formatCell('infinity', 'timestamp with time zone')).toEqual({ text: 'infinity', raw: null })
    expect(formatCell('infinity', 'date')).toEqual({ text: 'infinity', raw: null })
    expect(formatCell('2026-10-06', 'text')).toEqual({ text: '2026-10-06', raw: null })
  })
})
