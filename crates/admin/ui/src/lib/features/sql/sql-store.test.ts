import { beforeEach, describe, expect, it } from 'vitest'
import { sqlStore } from './sql-store.svelte'

// No localStorage in Node: the store works in memory only (the same path as
// a browser in private mode).
beforeEach(() => {
  for (const q of [...sqlStore.saved]) sqlStore.remove(q.id)
  sqlStore.open('')
})

describe('saved queries', () => {
  it('keeps edits to one query when opening and editing another', () => {
    sqlStore.setDraft('select 1')
    const one = sqlStore.save('one')
    sqlStore.setDraft('select 11')
    sqlStore.open('select 2')
    const two = sqlStore.save('two')
    sqlStore.setDraft('select 22')
    sqlStore.openSaved(one.id)
    expect(sqlStore.draft).toBe('select 11')
    expect(sqlStore.dirty).toBe(true)
    sqlStore.openSaved(two.id)
    expect(sqlStore.draft).toBe('select 22')
  })

  it('preserves loose work when opening a template and a new query', () => {
    sqlStore.setDraft('select expensive_work')
    const original = sqlStore.activeDraftId
    sqlStore.open('select template')
    sqlStore.open('')
    sqlStore.openDraft(original)
    expect(sqlStore.draft).toBe('select expensive_work')
    expect(sqlStore.looseDrafts.some((draft) => draft.sql === 'select template')).toBe(true)
  })

  it('saving with no open query creates one and starts editing it', () => {
    sqlStore.setDraft('select 1')
    const query = sqlStore.save('one')
    expect(sqlStore.saved).toHaveLength(1)
    expect(sqlStore.currentId).toBe(query.id)
    expect(sqlStore.dirty).toBe(false)
  })

  it('editing marks pending changes and saving updates the same query', () => {
    const query = sqlStore.save('one')
    sqlStore.setDraft('select 2')
    expect(sqlStore.dirty).toBe(true)
    sqlStore.save('one')
    expect(sqlStore.saved).toHaveLength(1)
    expect(sqlStore.saved[0]).toMatchObject({ id: query.id, sql: 'select 2' })
  })

  it('"save as new" keeps the original', () => {
    sqlStore.setDraft('select 1')
    sqlStore.save('original')
    sqlStore.setDraft('select 1 + 1')
    sqlStore.save('copy', true)
    expect(sqlStore.saved.map((q) => q.name).sort()).toEqual(['copy', 'original'])
    expect(sqlStore.current?.name).toBe('copy')
  })

  it('deleting the open query falls back to a loose draft', () => {
    const query = sqlStore.save('tmp')
    sqlStore.remove(query.id)
    expect(sqlStore.currentId).toBeNull()
    expect(sqlStore.current).toBeNull()
  })
})

describe('history', () => {
  it('no duplicates, most recent first, at most 20', () => {
    for (let i = 0; i < 25; i++) sqlStore.remember(`select ${i}`)
    sqlStore.remember('select 3')
    expect(sqlStore.history).toHaveLength(20)
    expect(sqlStore.history[0]).toBe('select 3')
    expect(sqlStore.history.filter((h) => h === 'select 3')).toHaveLength(1)
  })
})
