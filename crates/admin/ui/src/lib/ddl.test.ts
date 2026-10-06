import { describe, expect, it } from 'vitest'
import { columnChanges, grantChanges, policyFields, toColumnDef, toPolicyDef, type ColumnInfo } from './ddl'

const info = (overrides: Partial<ColumnInfo> = {}): ColumnInfo => ({
  name: 'texto',
  data_type: 'text',
  nullable: true,
  default: null,
  identity: null,
  generated: false,
  primary_key: false,
  unique: null,
  references: null,
  comment: null,
  ...overrides,
})

describe('diferença entre colunas', () => {
  it('sem mudanças, nenhuma ação', () => {
    expect(columnChanges(info(), toColumnDef(info()))).toEqual([])
  })

  it('cada campo alterado vira uma ação, com o rename por último', () => {
    const edited = {
      ...toColumnDef(info()),
      name: 'conteudo',
      data_type: 'varchar(200)',
      nullable: false,
      default: "''",
      unique: true,
      comment: 'corpo',
    }
    expect(columnChanges(info(), edited).map((a) => a.action)).toEqual([
      'set_type',
      'set_nullable',
      'set_default',
      'set_unique',
      'set_column_comment',
      'rename_column',
    ])
  })

  it('default vazio ou só espaços remove o DEFAULT', () => {
    const edited = { ...toColumnDef(info({ default: '0' })), default: '  ' }
    expect(columnChanges(info({ default: '0' }), edited)).toEqual([
      { action: 'set_default', column: 'texto', default: null },
    ])
  })

  it('não mexe no DEFAULT de identity ou coluna gerada', () => {
    const identity = info({ identity: 'always' })
    expect(columnChanges(identity, { ...toColumnDef(identity), default: '1' })).toEqual([])
  })

  it('chave estrangeira: troca, remoção e on delete do pg_catalog', () => {
    const fk = info({
      references: { table: 'clientes', column: 'id', on_delete: 'set null', constraint: 'fk' },
    })
    expect(toColumnDef(fk).references).toEqual({ table: 'clientes', column: 'id', on_delete: 'set_null' })
    expect(columnChanges(fk, toColumnDef(fk))).toEqual([])
    expect(columnChanges(fk, { ...toColumnDef(fk), references: null })).toEqual([
      { action: 'set_reference', column: 'texto', reference: null },
    ])
  })
})

describe('GRANTs', () => {
  it('só as roles que mudaram geram ação', () => {
    const before = [
      { role: 'anon' as const, privileges: ['select' as const] },
      { role: 'authenticated' as const, privileges: [] },
    ]
    expect(
      grantChanges(before, [
        { role: 'anon', privileges: ['select'] },
        { role: 'authenticated', privileges: ['select', 'insert'] },
      ]),
    ).toEqual([{ action: 'set_grants', grant: { role: 'authenticated', privileges: ['select', 'insert'] } }])
  })
})

describe('policies', () => {
  it('campos de cada comando', () => {
    expect(policyFields('select')).toEqual({ using: true, check: false })
    expect(policyFields('insert')).toEqual({ using: false, check: true })
    expect(policyFields('update')).toEqual({ using: true, check: true })
    expect(policyFields('delete')).toEqual({ using: true, check: false })
    expect(policyFields('all')).toEqual({ using: true, check: true })
  })

  it('converte a policy lida do Postgres', () => {
    expect(
      toPolicyDef({ name: 'p', command: 'SELECT', roles: ['public'], permissive: true, using: '(true)', check: null }),
    ).toEqual({ name: 'p', command: 'select', roles: [], permissive: true, using: '(true)', check: null })
  })
})
