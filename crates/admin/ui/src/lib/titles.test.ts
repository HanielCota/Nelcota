import { describe, expect, it } from 'vitest'
import { i18n } from './i18n/index.svelte'
import { crumbsFor, documentTitle } from './titles'

const title = (path: string) => documentTitle(crumbsFor(path))

describe('documentTitle', () => {
  it('names each page with the product last', () => {
    i18n.locale = 'pt-BR'
    expect(title('/')).toBe('Visão geral · Nelcota')
    expect(title('/sql')).toBe('Editor SQL · Nelcota')
    expect(title('/migrations')).toBe('Migrações · Nelcota')
    expect(title('/users/')).toBe('Usuários · Nelcota')
  })

  it('puts the most specific part first (fits the tab)', () => {
    i18n.locale = 'pt-BR'
    expect(title('/tables/pedidos')).toBe('pedidos · Tabelas · Nelcota')
    expect(title('/tables/pedidos/structure')).toBe('Estrutura · pedidos · Tabelas · Nelcota')
  })

  it('names unknown routes', () => {
    i18n.locale = 'pt-BR'
    expect(title('/nao-existe')).toBe('Página não encontrada · Nelcota')
  })

  it('follows the chosen language', () => {
    i18n.locale = 'en'
    expect(title('/tables/orders/structure')).toBe('Structure · orders · Tables · Nelcota')
    expect(title('/nope')).toBe('Page not found · Nelcota')
  })
})
