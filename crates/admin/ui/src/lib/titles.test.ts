import { describe, expect, it } from 'vitest'
import { crumbsFor, documentTitle } from './titles'

const title = (path: string) => documentTitle(crumbsFor(path))

describe('documentTitle', () => {
  it('nomeia cada página com o produto no fim', () => {
    expect(title('/')).toBe('Visão geral · Nelcota')
    expect(title('/sql')).toBe('Editor SQL · Nelcota')
    expect(title('/migrations')).toBe('Migrações · Nelcota')
    expect(title('/users/')).toBe('Usuários · Nelcota')
  })

  it('põe o mais específico primeiro (cabe na aba)', () => {
    expect(title('/tables/pedidos')).toBe('pedidos · Tabelas · Nelcota')
    expect(title('/tables/pedidos/structure')).toBe('Estrutura · pedidos · Tabelas · Nelcota')
  })

  it('identifica rota desconhecida', () => {
    expect(title('/nao-existe')).toBe('Página não encontrada · Nelcota')
  })
})
