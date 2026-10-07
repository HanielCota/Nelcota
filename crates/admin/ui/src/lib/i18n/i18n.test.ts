import { describe, expect, it } from 'vitest'
import { ApiError } from '../api'
import { detectLocale, errorMessage, i18n, t, translate } from './index.svelte'

describe('detectLocale', () => {
  it('prefers a saved choice', () => {
    expect(detectLocale('en', ['pt-BR'])).toBe('en')
    expect(detectLocale('pt-BR', ['en-US'])).toBe('pt-BR')
  })

  it('uses Portuguese for any Portuguese browser and English otherwise', () => {
    expect(detectLocale(null, ['pt-PT', 'en'])).toBe('pt-BR')
    expect(detectLocale(null, ['fr-FR', 'PT-br'])).toBe('pt-BR')
    expect(detectLocale(null, ['es-ES', 'en-GB'])).toBe('en')
    expect(detectLocale(null, [])).toBe('en')
    expect(detectLocale('klingon', ['de'])).toBe('en')
  })
})

describe('t', () => {
  it('follows the current language', () => {
    i18n.locale = 'pt-BR'
    expect(t('common.cancel')).toBe('Cancelar')
    i18n.locale = 'en'
    expect(t('common.cancel')).toBe('Cancel')
  })

  it('fills in parameters and leaves unknown ones visible', () => {
    expect(translate('en', 'common.page', { page: 3 })).toBe('Page 3')
    expect(translate('en', 'common.page')).toBe('Page {page}')
  })

  it('returns the key for a missing text (visible in development)', () => {
    expect(translate('en', 'common.nope')).toBe('common.nope')
  })
})

describe('errorMessage', () => {
  it('shows the server text when the code has no translation', () => {
    i18n.locale = 'pt-BR'
    expect(errorMessage(new ApiError('relation "x" does not exist', 400, 'not_a_known_code'))).toBe(
      'relation "x" does not exist',
    )
    expect(errorMessage(new Error('failed'))).toBe('failed')
    expect(errorMessage('text')).toBe('text')
  })
})
