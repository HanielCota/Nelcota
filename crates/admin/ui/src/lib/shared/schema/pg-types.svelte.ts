// Column types the server accepts (base list + the schema's enums). Loaded
// once per session (new enums show up after reloading the page).

import { api } from '$lib/api'
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
      // Without the list the field still accepts free text (the server validates).
      pending = undefined
    })
  return pending
}

/** Tables and columns of the exposed schema, to pick a foreign key. */
export async function loadSchemaColumns(): Promise<Record<string, string[]>> {
  const { tables } = await api.get<{ tables: Record<string, string[]> }>('/schema')
  // `auth.users` and the like stay out: the FK is for the exposed schema only.
  return Object.fromEntries(Object.entries(tables).filter(([name]) => !name.includes('.')))
}
