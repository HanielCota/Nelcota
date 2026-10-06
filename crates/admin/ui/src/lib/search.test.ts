import { describe, expect, it } from 'vitest'
import { commandScore } from './search'

describe('busca da paleta', () => {
  it('não casa letras soltas (o problema da busca fuzzy)', () => {
    expect(commandScore('página Editor SQL', 'pedidos')).toBe(0)
    expect(commandScore('tabela pedidos', 'pedidos')).toBeGreaterThan(0)
  })

  it('ignora acentos e maiúsculas', () => {
    expect(commandScore('página Visão geral', 'VISAO')).toBeGreaterThan(0)
    expect(commandScore('tabela usuarios', 'usuários')).toBeGreaterThan(0)
  })

  it('todos os termos precisam aparecer, em qualquer ordem', () => {
    expect(commandScore('consulta pedidos por status', 'status pedidos')).toBeGreaterThan(0)
    expect(commandScore('consulta pedidos por status', 'status clientes')).toBe(0)
  })

  it('começo de palavra vem antes de meio de palavra', () => {
    const start = commandScore('tabela pedidos', 'ped')
    const middle = commandScore('tabela itens_expedidos', 'pedidos')
    expect(start).toBeGreaterThan(middle)
    expect(commandScore('tabela itens_pedido', 'pedido')).toBe(1)
  })

  it('busca vazia mostra tudo; palavras-chave também contam', () => {
    expect(commandScore('qualquer', '  ')).toBe(1)
    expect(commandScore('ação sair', 'logout', ['logout'])).toBeGreaterThan(0)
  })
})
