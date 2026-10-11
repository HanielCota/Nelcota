import { describe, expect, it } from 'vitest'
import { Lexer, Marked, type TokensList } from 'marked'
import { guides, guideSource, guideTranslation, guideSearchText } from './content'
import { readMarkdown } from './markdown'
import { proseFragments, translateTokens, validateTranslations } from './prose'

function examples(tokens: TokensList) {
  const code: string[] = [], inline: string[] = [], links: string[] = []
  new Marked().walkTokens(tokens, token => {
    if (token.type === 'code') code.push(`${token.lang ?? ''}\n${token.text}`)
    if (token.type === 'codespan') inline.push(token.text)
    if (token.type === 'link' || token.type === 'image') links.push(token.href)
  })
  return { code, inline: inline.sort(), links: links.sort() }
}

describe('Portuguese guide contracts', () => {
  for (const guide of guides.filter(guide => !('language' in guide) && !('code' in guide) && guide.id !== 'openapi')) {
    it(`translates ${guide.id} with unchanged code, links and section URLs`, () => {
      const source = guideSource(guide), catalog = guideTranslation(guide, 'pt-BR')!
      expect(validateTranslations(source, catalog)).toEqual([])
      const tokens = Lexer.lex(source), original = examples(tokens)
      translateTokens(tokens, catalog)
      expect(examples(tokens)).toEqual(original)
      const english = readMarkdown(source, guide.source)
      const portuguese = readMarkdown(source, guide.source, catalog)
      expect(portuguese.filter(block => block.type === 'code')).toEqual(english.filter(block => block.type === 'code'))
      expect(portuguese.filter(block => block.type === 'heading').map(block => block.id)).toEqual(english.filter(block => block.type === 'heading').map(block => block.id))
      expect(guideTranslation(guide, 'en')).toBeUndefined()
    })
  }

  it('detects prose drift but immediately accepts updated source code', () => {
    const source = '# Guide\n\nUse `createClient()` and [API](api.md).\n\n```ts\n// Leave this comment alone\nconst value = 1\n```'
    const catalog = { Guide: 'Guia', 'Use `createClient()` and [API](api.md).': 'Use `createClient()` e [API](api.md).' }
    expect(validateTranslations(source, catalog)).toEqual([])
    expect(validateTranslations(source.replace('value = 1', 'value = 2'), catalog)).toEqual([])
    expect(validateTranslations(source.replace('Use `', 'Call `'), catalog)).toEqual(expect.arrayContaining([expect.stringContaining('Missing translation'), expect.stringContaining('Outdated translation')]))
    for (const changed of ['Use `other()` e [API](api.md).', 'Use `createClient()` e [API](elsewhere.md).']) {
      expect(validateTranslations(source, { ...catalog, 'Use `createClient()` and [API](api.md).': changed })).toContainEqual(expect.stringContaining('Changed inline code or link'))
    }
  })

  it('translates nested lists, quotes and table prose without touching their examples', () => {
    const source = '> A note with `code`.\n\n- First item\n  - Second item\n\n  ```sh\n  echo "English example"\n  ```\n\n| Column | Type |\n|---|---|\n| Name | `text` |'
    expect(proseFragments(source)).toEqual(['A note with `code`.', 'First item', 'Second item', 'Column', 'Type', 'Name'])
    const tokens = Lexer.lex(source), original = examples(tokens)
    translateTokens(tokens, { 'A note with `code`.': 'Uma nota com `code`.', 'First item': 'Primeiro item', 'Second item': 'Segundo item', Column: 'Coluna', Type: 'Tipo', Name: 'Nome' })
    const html = new Marked().parser(tokens)
    expect(html).toContain('Segundo item')
    expect(html).toContain('Uma nota')
    expect(html).toContain('<th>Coluna</th>')
    expect(examples(tokens)).toEqual(original)
  })

  it('searches Portuguese prose and original technical identifiers', () => {
    const auth = guides.find(guide => guide.id === 'auth')!
    expect(guideSearchText(auth, 'pt-BR')).toContain('caixa de entrada')
    expect(guideSearchText(auth, 'pt-BR')).toContain('NELCOTA_PASSWORD_RECOVERY_URL')
    expect(guideSearchText(auth, 'en')).not.toContain('caixa de entrada')
    const starter = guides.find(guide => guide.id === 'sdk-start')!
    expect(guideTranslation(starter, 'pt-BR')).toBeUndefined()
  })
})
