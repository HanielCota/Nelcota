// Busca da paleta de comandos. A busca fuzzy padrão aceita letras soltas em
// sequência ("pedidos" casava com "página Editor SQL"); aqui cada termo
// digitado precisa aparecer inteiro, sem diferenciar acentos e maiúsculas.

const normalize = (text: string) =>
  text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()

const escapeRegExp = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/**
 * Pontuação no formato do `filter` do bits-ui: 0 esconde o item; maior vem
 * primeiro. Termos no começo de uma palavra valem mais que no meio.
 */
export function commandScore(value: string, search: string, keywords: string[] = []): number {
  const terms = normalize(search).split(/\s+/).filter(Boolean)
  if (terms.length === 0) return 1
  const haystack = normalize([value, ...keywords].join(' '))
  if (!terms.every((term) => haystack.includes(term))) return 0
  const atWordStart = terms.every((term) => new RegExp(`(^|[\\s_./-])${escapeRegExp(term)}`).test(haystack))
  return atWordStart ? 1 : 0.5
}
