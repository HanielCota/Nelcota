import { Lexer, type Token, type Tokens } from 'marked'

export type TranslationCatalog = Record<string, string>
type Prose = Tokens.Heading | Tokens.Paragraph | Tokens.Text | Tokens.TableCell

function needsTranslation(text: string): boolean {
  const prose = (tokens: Token[]): string => tokens.map(token => token.type === 'codespan' || token.type === 'html' ? '' : 'tokens' in token && token.tokens ? prose(token.tokens) : 'text' in token ? token.text : '').join('')
  return /\p{L}/u.test(prose(Lexer.lexInline(text)))
}

/** Visit complete prose fragments, never code blocks or their comments. */
function visit(tokens: Token[], apply: (fragment: Prose) => void) {
  for (const token of tokens) {
    if (token.type === 'heading' || token.type === 'paragraph' || (token.type === 'text' && token.tokens)) {
      apply(token as Prose)
    } else if (token.type === 'table') {
      const table = token as Tokens.Table
      for (const cell of [...table.header, ...table.rows.flat()]) apply(cell)
    } else if (token.type === 'list') {
      for (const item of (token as Tokens.List).items) visit(item.tokens, apply)
    } else if (token.type === 'blockquote') visit((token as Tokens.Blockquote).tokens, apply)
  }
}

export function proseFragments(markdown: string): string[] {
  const fragments: string[] = []
  visit(Lexer.lex(markdown), fragment => { if (needsTranslation(fragment.text)) fragments.push(fragment.text) })
  return [...new Set(fragments)]
}

/** Inline code and destinations are part of the contract of a translation. */
export function protectedInline(markdown: string) {
  const code: string[] = []
  const links: string[] = []
  const tags: string[] = []
  function walk(tokens: Token[]) {
    for (const token of tokens) {
      if (token.type === 'codespan') code.push(token.text)
      if (token.type === 'link' || token.type === 'image') links.push(token.href)
      if (token.type === 'html') tags.push(token.text)
      if ('tokens' in token && token.tokens) walk(token.tokens)
    }
  }
  walk(Lexer.lexInline(markdown))
  // Reordering a sentence may move a code span without changing the example.
  return { code: code.sort(), links: links.sort(), tags: tags.sort() }
}

export function validateTranslations(markdown: string, catalog: TranslationCatalog): string[] {
  const fragments = proseFragments(markdown)
  const current = new Set(fragments)
  const errors: string[] = []
  for (const fragment of fragments) {
    const translated = catalog[fragment]
    if (typeof translated !== 'string' || !translated.trim()) errors.push(`Missing translation: ${fragment}`)
    else if (JSON.stringify(protectedInline(fragment)) !== JSON.stringify(protectedInline(translated))) errors.push(`Changed inline code or link: ${fragment}`)
  }
  for (const fragment of Object.keys(catalog)) if (!current.has(fragment)) errors.push(`Outdated translation: ${fragment}`)
  return errors
}

/** Translate the existing AST: code, links and document structure stay sourced from the original. */
export function translateTokens(tokens: Token[], catalog: TranslationCatalog) {
  visit(tokens, fragment => {
    if (!needsTranslation(fragment.text)) return
    const translated = catalog[fragment.text]
    if (translated === undefined) throw new Error(`Missing guide translation: ${fragment.text}`)
    fragment.text = translated
    fragment.tokens = Lexer.lexInline(translated)
  })
}
