export type Json = null | boolean | number | string | Json[] | { [key: string]: Json }
export type JsonObject = { [key: string]: Json }
export type OpenApiDocument = JsonObject & { openapi: string; paths: Record<string, JsonObject> }
export const object = (value: Json | undefined): JsonObject => value !== null && typeof value === 'object' && !Array.isArray(value) ? value : {}
export const text = (value: Json | undefined) => typeof value === 'string' ? value : ''
const methods = ['get', 'post', 'put', 'patch', 'delete', 'head', 'options']

export function isOpenApiDocument(value: unknown): value is OpenApiDocument {
  if (!value || typeof value !== 'object') return false
  const doc = value as Record<string, unknown>
  return typeof doc.openapi === 'string' && doc.openapi.startsWith('3.') && !!doc.paths && typeof doc.paths === 'object' && !Array.isArray(doc.paths)
    && Object.values(doc.paths).every(path => path !== null && typeof path === 'object' && !Array.isArray(path))
}

export function operations(doc: OpenApiDocument) {
  return Object.entries(doc.paths).flatMap(([path, item]) => methods.filter(method => item[method] && typeof item[method] === 'object' && !Array.isArray(item[method])).map(method => ({
    path, method: method.toUpperCase(), operation: object(item[method]),
  })))
}
