// Export of results already in the browser (SQL editor). Whole tables are
// exported by the server, streamed (`/tables/:name/export`).

type Cell = string | null

function csvField(value: Cell): string {
  if (value === null) return ''
  return /[",\r\n]|^\s|\s$/.test(value) ? `"${value.replaceAll('"', '""')}"` : value
}

/** CSV (RFC 4180) with a BOM, so Excel recognises UTF-8. NULL = empty field. */
export function toCsv(columns: readonly string[], rows: readonly (readonly Cell[])[]): string {
  const lines = [columns, ...rows].map((row) => row.map(csvField).join(','))
  return `﻿${lines.join('\r\n')}\r\n`
}

/** Array of objects; values stay as Postgres returned them (text). */
export function toJson(columns: readonly string[], rows: readonly (readonly Cell[])[]): string {
  const objects = rows.map((row) => Object.fromEntries(columns.map((c, i) => [c, row[i] ?? null])))
  return `${JSON.stringify(objects, null, 2)}\n`
}

export function downloadText(fileName: string, text: string, mime: string) {
  const url = URL.createObjectURL(new Blob([text], { type: mime }))
  const link = Object.assign(document.createElement('a'), { href: url, download: fileName })
  link.click()
  // The click already started the download; the URL can be released on the next tick.
  setTimeout(() => URL.revokeObjectURL(url), 0)
}
