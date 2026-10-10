import { describe, expect, it } from 'vitest'
import { environment } from './health.svelte'

describe('environment', () => {
  it('is local on this machine', () => {
    for (const host of ['localhost', '127.0.0.1', '::1', 'app.localhost']) expect(environment(host)).toEqual({ local: true })
  })

  it('names the host anywhere else', () => {
    expect(environment('db.example.com')).toEqual({ local: false, host: 'db.example.com' })
  })
})
