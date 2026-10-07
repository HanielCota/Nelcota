// Modelos de policy mais comuns. Os de "dono" usam a coluna que guarda o
// usuário (uuid), comparada com auth.uid().

import type { ColumnInfo, PolicyDef } from './ddl'

export interface PolicyTemplate {
  label: string
  description: string
  build: (ownerColumn: string) => Omit<PolicyDef, 'permissive'>
}

const owner = (column: string) => `${quoteIfNeeded(column)} = auth.uid()`

/** Aspas só quando o nome não é um identificador simples em minúsculas. */
export function quoteIfNeeded(name: string): string {
  return /^[a-z_][a-z0-9_]*$/.test(name) ? name : `"${name.replaceAll('"', '""')}"`
}

export const POLICY_TEMPLATES: readonly PolicyTemplate[] = [
  {
    label: 'Leitura pública',
    description: 'Qualquer um lê todas as linhas, logado ou não.',
    build: () => ({ name: 'leitura pública', command: 'select', roles: ['anon', 'authenticated'], using: 'true', check: null }),
  },
  {
    label: 'Usuários logados leem',
    description: 'Só quem está logado lê; anônimos não.',
    build: () => ({ name: 'logados leem', command: 'select', roles: ['authenticated'], using: 'true', check: null }),
  },
  {
    label: 'Dono lê as próprias linhas',
    description: 'Cada usuário vê só as linhas em que é o dono.',
    build: (column) => ({ name: 'dono lê', command: 'select', roles: ['authenticated'], using: owner(column), check: null }),
  },
  {
    label: 'Dono cria as próprias linhas',
    description: 'Só aceita linhas novas em nome de quem está logado.',
    build: (column) => ({ name: 'dono cria', command: 'insert', roles: ['authenticated'], using: null, check: owner(column) }),
  },
  {
    label: 'Dono altera as próprias linhas',
    description: 'Altera só o que é seu, sem passar a linha para outro dono.',
    build: (column) => ({
      name: 'dono altera',
      command: 'update',
      roles: ['authenticated'],
      using: owner(column),
      check: owner(column),
    }),
  },
  {
    label: 'Dono apaga as próprias linhas',
    description: 'Apaga só o que é seu.',
    build: (column) => ({ name: 'dono apaga', command: 'delete', roles: ['authenticated'], using: owner(column), check: null }),
  },
  {
    label: 'Dono faz tudo nas próprias linhas',
    description: 'Lê, cria, altera e apaga só o que é seu (uma policy para tudo).',
    build: (column) => ({
      name: 'dono faz tudo',
      command: 'all',
      roles: ['authenticated'],
      using: owner(column),
      check: owner(column),
    }),
  },
]

const OWNER_NAMES = ['user_id', 'usuario_id', 'dono', 'dono_id', 'owner', 'owner_id', 'autor_id', 'criado_por']

/** Coluna mais provável de guardar o dono: uuid com nome conhecido, ou o primeiro uuid que não é a PK. */
export function guessOwnerColumn(columns: readonly ColumnInfo[]): string {
  const uuids = columns.filter((c) => c.data_type === 'uuid')
  return (
    uuids.find((c) => OWNER_NAMES.includes(c.name.toLowerCase()))?.name ??
    uuids.find((c) => !c.primary_key)?.name ??
    'user_id'
  )
}
