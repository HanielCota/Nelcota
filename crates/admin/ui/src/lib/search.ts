// Command palette search. The default fuzzy search accepts scattered letters
// in order ("orders" matched "page Our Data Records"); here every typed term
// must appear whole, ignoring accents and case.

const normalize = (text: string) =>
  text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()

const escapeRegExp = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/**
 * Score in the format of bits-ui's `filter`: 0 hides the item; higher comes
 * first. Terms at the start of a word score higher than in the middle.
 */
export function commandScore(value: string, search: string, keywords: string[] = []): number {
  const terms = normalize(search).split(/\s+/).filter(Boolean)
  if (terms.length === 0) return 1
  const haystack = normalize([value, ...keywords].join(' '))
  if (!terms.every((term) => haystack.includes(term))) return 0
  const atWordStart = terms.every((term) => new RegExp(`(^|[\\s_./-])${escapeRegExp(term)}`).test(haystack))
  return atWordStart ? 1 : 0.5
}
