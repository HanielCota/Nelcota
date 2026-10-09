import { describe, expect, it } from 'vitest'
import { access, describePolicy, ownerColumn } from './plain'

describe('access', () => {
  it('reads grants as plain levels', () => {
    expect(access([])).toEqual({ level: 'none', verbs: [] })
    expect(access(['SELECT'])).toEqual({ level: 'read', verbs: ['read'] })
    expect(access(['DELETE', 'select', 'UPDATE', 'INSERT']).level).toBe('full')
    expect(access(['INSERT', 'SELECT'])).toEqual({ level: 'partial', verbs: ['read', 'create'] })
  })
})

describe('ownerColumn', () => {
  it('finds the column compared with auth.uid() as Postgres prints it', () => {
    expect(ownerColumn('(owner = auth.uid())')).toBe('owner')
    expect(ownerColumn('(auth.uid() = user_id)')).toBe('user_id')
    expect(ownerColumn('("Dono" = auth.uid())')).toBe('Dono')
    expect(ownerColumn('(user_id = ( SELECT auth.uid() AS uid))')).toBe('user_id')
    expect(ownerColumn('(owner = auth.uid()) AND (status = 1)')).toBeNull()
    expect(ownerColumn('true')).toBeNull()
    expect(ownerColumn(null)).toBeNull()
  })
})

describe('describePolicy', () => {
  const rule = (command: string, roles: string[], using: string | null, check: string | null) =>
    describePolicy({ command, roles, using, check })

  it('recognizes public and signed-in reading', () => {
    expect(rule('SELECT', ['anon', 'authenticated'], 'true', null).kind).toBe('everyoneReads')
    expect(rule('SELECT', [], '(true)', null).kind).toBe('everyoneReads')
    expect(rule('SELECT', ['authenticated'], 'true', null).kind).toBe('signedInReads')
  })

  it('recognizes owner rules per command', () => {
    expect(rule('ALL', ['authenticated'], '(owner = auth.uid())', '(owner = auth.uid())')).toEqual({
      kind: 'owner',
      command: 'all',
      column: 'owner',
    })
    expect(rule('INSERT', ['authenticated'], null, '(user_id = auth.uid())')).toMatchObject({ command: 'insert' })
    expect(rule('DELETE', ['authenticated'], '(user_id = auth.uid())', null)).toMatchObject({ command: 'delete' })
  })

  it('leaves anything else as a custom rule', () => {
    // Different columns on each side, a public owner rule, extra conditions.
    expect(rule('UPDATE', ['authenticated'], '(a = auth.uid())', '(b = auth.uid())').kind).toBe('custom')
    expect(rule('ALL', ['anon'], '(owner = auth.uid())', null).kind).toBe('custom')
    expect(rule('SELECT', ['authenticated'], '(published AND (owner = auth.uid()))', null).kind).toBe('custom')
    expect(rule('INSERT', ['authenticated'], '(owner = auth.uid())', null).kind).toBe('custom')
  })
})
