// Exportação de resultados que já estão no navegador (editor SQL). Tabelas
// inteiras são exportadas pelo servidor, em fluxo (`/tables/:nome/export`).

type Cell = string | null

function csvField(value: Cell): string {
  if (value === null) return ''
  return /[",\r\n]|^\s|\s$/.test(value) ? `"${value.replaceAll('"', '""')}"` : value
}

/** CSV (RFC 4180) com BOM, para o Excel reconhecer UTF-8. NULL = campo vazio. */
export function toCsv(columns: readonly string[], rows: readonly (readonly Cell[])[]): string {
  const lines = [columns, ...rows].map((row) => row.map(csvField).join(','))
  return `﻿${lines.join('\r\n')}\r\n`
}

/** Array de objetos; os valores ficam como o Postgres devolveu (texto). */
export function toJson(columns: readonly string[], rows: readonly (readonly Cell[])[]): string {
  const objects = rows.map((row) => Object.fromEntries(columns.map((c, i) => [c, row[i] ?? null])))
  return `${JSON.stringify(objects, null, 2)}\n`
}

export function downloadText(fileName: string, text: string, mime: string) {
  const url = URL.createObjectURL(new Blob([text], { type: mime }))
  const link = Object.assign(document.createElement('a'), { href: url, download: fileName })
  link.click()
  // O clique já iniciou o download; a URL pode ser liberada no próximo tick.
  setTimeout(() => URL.revokeObjectURL(url), 0)
}
