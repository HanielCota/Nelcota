import { beforeEach, describe, expect, it } from 'vitest'
import { sqlStore } from './sql-store.svelte'

// Sem localStorage no Node: o store funciona só em memória (o mesmo caminho
// de um navegador em modo privado).
beforeEach(() => {
  for (const q of [...sqlStore.saved]) sqlStore.remove(q.id)
  sqlStore.open('')
})

describe('consultas salvas', () => {
  it('salvar sem consulta aberta cria uma nova e passa a editá-la', () => {
    sqlStore.setDraft('select 1')
    const query = sqlStore.save('um')
    expect(sqlStore.saved).toHaveLength(1)
    expect(sqlStore.currentId).toBe(query.id)
    expect(sqlStore.dirty).toBe(false)
  })

  it('editar marca alterações pendentes e salvar atualiza a mesma', () => {
    const query = sqlStore.save('um')
    sqlStore.setDraft('select 2')
    expect(sqlStore.dirty).toBe(true)
    sqlStore.save('um')
    expect(sqlStore.saved).toHaveLength(1)
    expect(sqlStore.saved[0]).toMatchObject({ id: query.id, sql: 'select 2' })
  })

  it('"salvar como nova" mantém a original', () => {
    sqlStore.setDraft('select 1')
    sqlStore.save('original')
    sqlStore.setDraft('select 1 + 1')
    sqlStore.save('cópia', true)
    expect(sqlStore.saved.map((q) => q.name).sort()).toEqual(['cópia', 'original'])
    expect(sqlStore.current?.name).toBe('cópia')
  })

  it('apagar a consulta aberta volta para o rascunho solto', () => {
    const query = sqlStore.save('tmp')
    sqlStore.remove(query.id)
    expect(sqlStore.currentId).toBeNull()
    expect(sqlStore.current).toBeNull()
  })
})

describe('histórico', () => {
  it('sem repetidos, mais recente primeiro, no máximo 20', () => {
    for (let i = 0; i < 25; i++) sqlStore.remember(`select ${i}`)
    sqlStore.remember('select 3')
    expect(sqlStore.history).toHaveLength(20)
    expect(sqlStore.history[0]).toBe('select 3')
    expect(sqlStore.history.filter((h) => h === 'select 3')).toHaveLength(1)
  })
})
