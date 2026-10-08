import * as validators from './generated/validators.js'

type Validator = (value: unknown) => boolean
const validate = validators as Record<string, Validator>

/** Select the wire contract centrally, independent of a caller's generic type. */
export function responseContract(method: string, path: string): Validator | undefined {
  const route = path.split('?')[0]
  if (method === 'GET') {
    const exact: Record<string, string> = {
      '/overview': 'Overview', '/tables': 'TablesResponse', '/schema': 'SchemaResponse', '/users': 'UsersResponse',
      '/policies': 'PoliciesData', '/projects': 'ProjectsData', '/projects/status': 'ProjectsStatusData', '/migrations': 'MigrationsData',
      '/storage': 'StorageOverview', '/types': 'TypesResponse',
    }
    if (exact[route]) return validate[exact[route]]
    if (/^\/tables\/[^/]+\/structure$/.test(route)) return validate.Structure
    if (/^\/tables\/[^/]+$/.test(route)) return validate.TableData
    if (/^\/storage\/buckets\/[^/]+\/objects$/.test(route)) return validate.StorageListing
  }
  if (method === 'POST' && route === '/sql') return validate.SqlResponse
  if (method === 'POST' && route === '/migrations') return validate.ExportedMigration
  if (['POST', 'PATCH', 'PUT', 'DELETE'].includes(method) &&
      (route === '/tables' || /^\/tables\/[^/]+(?:\/policies(?:\/[^/]+)?)?$/.test(route))) return validate.DdlResult
  return undefined
}
