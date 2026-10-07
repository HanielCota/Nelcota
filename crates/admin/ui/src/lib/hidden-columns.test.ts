import { describe, expect, it } from 'vitest'
import { HiddenColumns } from './hidden-columns.svelte'

describe('colunas ocultas', () => {
  it('ocultar, mostrar, alternar e mostrar todas', () => {
    const hidden = new HiddenColumns()
    hidden.load('pedidos')
    hidden.hide('criado_em')
    hidden.hide('criado_em')
    expect(hidden.names).toEqual(['criado_em'])
    hidden.toggle('status')
    expect(hidden.has('status')).toBe(true)
    hidden.toggle('status')
    expect(hidden.has('status')).toBe(false)
    hidden.showAll()
    expect(hidden.names).toEqual([])
  })

  it('esquece colunas que deixaram de existir', () => {
    const hidden = new HiddenColumns()
    hidden.load('pedidos')
    hidden.hide('antiga')
    hidden.hide('total')
    hidden.prune(['id', 'total'])
    expect(hidden.names).toEqual(['total'])
  })

  it('cada tabela tem as suas (sem localStorage, começa vazio)', () => {
    const hidden = new HiddenColumns()
    hidden.load('clientes')
    expect(hidden.names).toEqual([])
  })
})
