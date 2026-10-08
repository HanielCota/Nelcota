import { describe, expect, it } from 'vitest'
import { baseName, encodePath, folderTrail, formatBytes, parseTypes, previewKind, publicUrl, validBucketName } from './files'

describe('formatBytes', () => {
  it('picks a unit', () => {
    expect(formatBytes(0, 'en')).toBe('0 B')
    expect(formatBytes(512, 'en')).toBe('512 B')
    expect(formatBytes(1536, 'en')).toBe('1.5 kB')
    expect(formatBytes(5 * 1024 * 1024, 'en')).toBe('5 MB')
    expect(formatBytes(1536, 'pt-BR')).toBe('1,5 kB')
  })
})

describe('paths', () => {
  it('splits a prefix into a trail', () => {
    expect(folderTrail('')).toEqual([])
    expect(folderTrail('a/b/')).toEqual([
      { name: 'a', prefix: 'a/' },
      { name: 'b', prefix: 'a/b/' },
    ])
  })

  it('names and encodes', () => {
    expect(baseName('a/b/c.png')).toBe('c.png')
    expect(baseName('c.png')).toBe('c.png')
    expect(encodePath('a b/ç#?.png')).toBe('a%20b/%C3%A7%23%3F.png')
    expect(publicUrl('https://x.com/', 'avatars', 'u/me.png')).toBe(
      'https://x.com/storage/v1/object/public/avatars/u/me.png',
    )
  })
})

describe('bucket settings', () => {
  it('parses type lists', () => {
    expect(parseTypes(' Image/*, application/pdf\ntext/plain ')).toEqual(['image/*', 'application/pdf', 'text/plain'])
    expect(parseTypes('  ')).toEqual([])
  })

  it('checks bucket names', () => {
    expect(validBucketName('avatars')).toBe(true)
    expect(validBucketName('user-files_2')).toBe(true)
    for (const bad of ['', 'Avatars', '-a', 'a b', 'public', 'sign', 'list', 'x'.repeat(64)]) {
      expect(validBucketName(bad)).toBe(false)
    }
  })
})

describe('file previews', () => {
  it('limits previews to passive formats', () => {
    expect(previewKind('image/png', 512)).toBe('image')
    expect(previewKind('application/pdf', 512)).toBe('pdf')
    expect(previewKind('application/json', 512)).toBe('text')
    expect(previewKind('text/html', 512)).toBeNull()
    expect(previewKind('image/svg+xml', 512)).toBeNull()
  })

  it('limits large binary and text previews independently', () => {
    expect(previewKind('text/plain', 1024 * 1024 + 1)).toBeNull()
    expect(previewKind('image/png', 20 * 1024 * 1024)).toBe('image')
    expect(previewKind('image/png', 20 * 1024 * 1024 + 1)).toBeNull()
  })
})
