import catalog from '../../../../../../../docs/i18n/pt-BR/openapi.json'
import { object, text, operations, type Json, type JsonObject, type OpenApiDocument } from './openapi'

const translations: Record<string, string> = catalog
const group = Object.keys(catalog).find(key => key.startsWith('Group of filters'))!
const fill = (template: string, logic: string) => template.replace('{}', logic.toUpperCase()).replaceAll('{logic}', logic)
function description(value: Json | undefined): Json | undefined {
  if (typeof value !== 'string') return value
  if (Object.hasOwn(translations, value)) return translations[value]
  for (const logic of ['and', 'or']) if (value === fill(group, logic)) return fill(translations[group], logic)
  return value
}

/** Localize generated prose for presentation; keep the downloaded contract untouched. */
export function translateOpenApi(source: OpenApiDocument, locale: string): OpenApiDocument {
  if (locale !== 'pt-BR') return source
  const doc = structuredClone(source)
  const info = object(doc.info)
  for (const key of ['title', 'description']) if (key in info) info[key] = description(info[key])!
  for (const { operation } of operations(doc)) {
    for (const parameter of Array.isArray(operation.parameters) ? operation.parameters : []) {
      const item = object(parameter)
      if ('description' in item) item.description = description(item.description)!
    }
    for (const response of Object.values(object(operation.responses))) {
      const item = object(response)
      if ('description' in item) item.description = description(item.description)!
    }
  }
  function schema(item: JsonObject) {
    const value = text(item.description)
    // Preserve user-authored comments; only the generated type suffix changes.
    if (value) item.description = value.replace(/(^|\n\n)Postgres type: ([^\n]+)$/, (_, prefix, type) => `${prefix}${translations['Postgres type: {}'].replace('{}', type)}`)
    for (const property of Object.values(object(item.properties))) schema(object(property))
    if (item.items) schema(object(item.items))
    for (const key of ['oneOf', 'anyOf', 'allOf']) if (Array.isArray(item[key])) for (const child of item[key]) schema(object(child))
  }
  for (const model of Object.values(object(object(doc.components).schemas))) schema(object(model))
  return doc
}
