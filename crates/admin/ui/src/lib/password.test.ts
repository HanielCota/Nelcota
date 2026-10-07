import { describe, expect, it } from 'vitest'
import { generatePassword, passwordProblem } from './password'

describe('senha gerada', () => {
  it('tem o tamanho pedido e só caracteres não ambíguos', () => {
    const password = generatePassword(64)
    expect(password).toHaveLength(64)
    expect(password).toMatch(/^[a-km-zA-HJ-NP-Z2-9_-]+$/)
  })

  it('descarta bytes que causariam viés de módulo', () => {
    // 59 símbolos: bytes ≥ 236 (256 - 256 % 59) são descartados.
    const bytes = [250, 240, 236, 0, 58]
    const random = () => bytes.shift()!
    expect(generatePassword(2, random)).toBe('a_')
  })

  it('distribuição uniforme o bastante', () => {
    const counts = new Map<string, number>()
    for (const c of generatePassword(59 * 400)) counts.set(c, (counts.get(c) ?? 0) + 1)
    expect(counts.size).toBe(59)
    for (const n of counts.values()) expect(n).toBeGreaterThan(250)
  })
})

describe('regra de tamanho', () => {
  it('igual à do servidor (conta caracteres, não bytes)', () => {
    expect(passwordProblem('1234567')).toBe('mínimo de 8 caracteres')
    expect(passwordProblem('çççççççç')).toBeNull()
    expect(passwordProblem('x'.repeat(257))).toBe('máximo de 256 caracteres')
  })
})
