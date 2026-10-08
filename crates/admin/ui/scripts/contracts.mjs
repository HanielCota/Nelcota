import { readFile, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
const require = createRequire(import.meta.url)
const Ajv = require('ajv/dist/2020').default
const standalone = require('ajv/dist/standalone').default
const schemas = JSON.parse(await readFile(new URL('../src/lib/generated/schemas.json', import.meta.url)))
const ajv = new Ajv({ code: { source: true, esm: true }, strict: false, validateFormats: false })
const refs = {}
const names = ['Overview', 'TablesResponse', 'SchemaResponse', 'UsersResponse', 'PoliciesData', 'ProjectsData',
  'ProjectsStatusData', 'MigrationsData', 'StorageOverview', 'TypesResponse', 'Structure', 'TableData', 'StorageListing',
  'SqlResponse', 'ExportedMigration', 'DdlResult']
for (const name of names) {
  const schema = schemas[name]
  ajv.addSchema(schema, name)
  refs[name] = name
}
// Compile at build time: the browser never uses eval (the panel has a strict CSP).
await writeFile(new URL('../src/lib/generated/validators.js', import.meta.url), '// @ts-nocheck\n// Generated. Do not edit.\n' + standalone(ajv, refs))
await writeFile(new URL('../src/lib/generated/validators.d.ts', import.meta.url), '// Generated. Do not edit.\n' + names.map(name => `export function ${name}(value: unknown): boolean;\n`).join(''))
