import { describe, expect, it } from 'vitest'
import { POLICY_TEMPLATES, guessOwnerColumn, quoteIfNeeded } from './policy-templates'
import { policyFields, type ColumnInfo } from './ddl'

const column = (name: string, data_type: string, primary_key = false): ColumnInfo => ({
  name,
  data_type,
  primary_key,
  nullable: true,
  default: null,
  identity: null,
  generated: false,
  unique: null,
  references: null,
  comment: null,
})

describe('modelos de policy', () => {
  it('cada modelo usa só as expressões que o comando aceita', () => {
    for (const template of POLICY_TEMPLATES) {
      const policy = template.build('dono')
      const fields = policyFields(policy.command)
      expect(policy.using !== null, `${template.label}: USING`).toBe(fields.using)
      if (!fields.check) expect(policy.check, `${template.label}: WITH CHECK`).toBeNull()
    }
  })

  it('modelos de dono comparam a coluna com auth.uid()', () => {
    const insert = POLICY_TEMPLATES.find((t) => t.label.startsWith('Dono cria'))!.build('autor_id')
    expect(insert.check).toBe('autor_id = auth.uid()')
  })

  it('aspas só quando precisa', () => {
    expect(quoteIfNeeded('user_id')).toBe('user_id')
    expect(quoteIfNeeded('Dono')).toBe('"Dono"')
    expect(quoteIfNeeded('dono id')).toBe('"dono id"')
  })
})

describe('coluna do dono', () => {
  it('prefere uuid com nome conhecido', () => {
    expect(guessOwnerColumn([column('id', 'uuid', true), column('ref', 'uuid'), column('dono', 'uuid')])).toBe('dono')
  })

  it('senão, o primeiro uuid que não é a PK; senão, user_id', () => {
    expect(guessOwnerColumn([column('id', 'uuid', true), column('autor', 'uuid')])).toBe('autor')
    expect(guessOwnerColumn([column('id', 'bigint', true)])).toBe('user_id')
  })
})
