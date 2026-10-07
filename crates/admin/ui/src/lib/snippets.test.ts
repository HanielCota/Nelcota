import { describe, expect, it } from 'vitest'
import { authSnippets, sampleRow, sampleValue, tableSnippets, type SnippetColumn } from './snippets'

const col = (name: string, type: string, extra: Partial<SnippetColumn> = {}): SnippetColumn => ({
  name,
  type,
  has_default: false,
  generated: false,
  nullable: true,
  ...extra,
})

const columns = [
  col('id', 'bigint', { has_default: true, nullable: false }),
  col('titulo', 'text', { nullable: false }),
  col('feito', 'boolean'),
  col('slug', 'text', { generated: true }),
  col('criado_em', 'timestamp with time zone', { has_default: true }),
]

describe('valores e corpo de exemplo', () => {
  it('valor plausível por tipo', () => {
    expect(sampleValue('integer')).toBe(1)
    expect(sampleValue('uuid')).toMatch(/^0{8}-/)
    expect(sampleValue('text[]')).toEqual([])
    expect(sampleValue('jsonb')).toEqual({})
  })

  it('corpo deixa de fora DEFAULT e geradas, obrigatórias primeiro', () => {
    expect(sampleRow(columns)).toEqual({ titulo: 'exemplo', feito: true })
  })
})

describe('exemplos da tabela', () => {
  const snippets = tableSnippets('https://loja.exemplo.com', 'tarefas', columns)

  it('cobre listar, filtrar, inserir, atualizar e apagar', () => {
    expect(snippets.map((s) => s.id)).toEqual(['list', 'filter', 'insert', 'update', 'delete'])
  })

  it('URL, filtro e corpo corretos', () => {
    const byId = Object.fromEntries(snippets.map((s) => [s.id, s]))
    expect(byId.list.code.curl).toContain("'https://loja.exemplo.com/rest/v1/tarefas?select=*&limit=20'")
    expect(byId.filter.code.curl).toContain('titulo=eq.exemplo')
    expect(byId.insert.code.curl).toContain(`-d '{"titulo":"exemplo","feito":true}'`)
    expect(byId.insert.code.js).toContain("Prefer: 'return=representation'")
  })

  it('aspas simples no shell são escapadas', () => {
    const quoted = tableSnippets('http://x', 't', [col("nome d'água", 'text', { nullable: false })])
    expect(quoted.find((s) => s.id === 'insert')!.code.curl).toContain(`'{"nome d'\\''água":"exemplo"}'`)
  })

  it('nome de tabela vai codificado na URL', () => {
    expect(tableSnippets('http://x', 'pedidos itens', [])[0].code.js).toContain('/rest/v1/pedidos%20itens')
  })
})

describe('exemplos de autenticação', () => {
  it('signup, login com senha e refresh', () => {
    const snippets = authSnippets('https://loja.exemplo.com')
    expect(snippets.map((s) => s.id)).toEqual(['signup', 'login', 'refresh'])
    expect(snippets[1].code.curl).toContain('/auth/v1/token?grant_type=password')
  })
})
