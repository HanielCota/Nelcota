// Tipos de coluna que o servidor aceita (lista base + enums do schema).
// Carregados uma vez por sessão (enums novos aparecem ao recarregar a página).

import { api } from './api'
import { ddl } from './ddl'

export const pgTypes = $state({ base: [] as string[], enums: [] as string[], loaded: false })

let pending: Promise<void> | undefined

export function loadTypes(): Promise<void> {
  pending ??= ddl
    .types()
    .then((types) => {
      pgTypes.base = types.base
      pgTypes.enums = types.enums
      pgTypes.loaded = true
    })
    .catch(() => {
      // Sem a lista, o campo continua aceitando texto livre (o servidor valida).
      pending = undefined
    })
  return pending
}

/** Tabelas e colunas do schema exposto, para escolher a chave estrangeira. */
export async function loadSchemaColumns(): Promise<Record<string, string[]>> {
  const { tables } = await api.get<{ tables: Record<string, string[]> }>('/schema')
  // `auth.users` e afins ficam de fora: a FK é só para o schema exposto.
  return Object.fromEntries(Object.entries(tables).filter(([name]) => !name.includes('.')))
}
