// Exibição de valores na grade. Só a exibição: edição, exportação e o
// tooltip continuam com o texto exato que o Postgres devolveu.

export interface CellDisplay {
  text: string
  /** Valor original, para o tooltip (`null` quando é igual ao exibido). */
  raw: string | null
}

const two = (n: string) => n.padStart(2, '0')

/** `2026-10-06T22:26:15.177028` → partes, sem conversão de fuso. */
const WALL_CLOCK = /^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2}))?)?/

/**
 * Datas no formato brasileiro. `timestamptz` vai para o fuso de quem está
 * vendo (é um instante); `timestamp` e `date` ficam como gravados (não têm
 * fuso). Valor que não reconhecer sai como veio.
 */
export function formatCell(value: string, type: string, options: { timeZone?: string } = {}): CellDisplay {
  if (type === 'timestamp with time zone') {
    const instant = new Date(value)
    if (Number.isNaN(instant.getTime())) return { text: value, raw: null }
    const text = new Intl.DateTimeFormat('pt-BR', {
      dateStyle: 'short',
      timeStyle: 'medium',
      timeZone: options.timeZone,
    }).format(instant)
    return { text, raw: value }
  }
  if (type === 'timestamp without time zone' || type === 'date') {
    const m = WALL_CLOCK.exec(value)
    if (!m) return { text: value, raw: null }
    const [, year, month, day, hour, minute, second] = m
    const date = `${day}/${month}/${year}`
    if (type === 'date' || hour === undefined) return { text: date, raw: value }
    return { text: `${date}, ${two(hour)}:${minute}:${second ?? '00'}`, raw: value }
  }
  return { text: value, raw: null }
}
