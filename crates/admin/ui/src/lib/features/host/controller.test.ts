import { describe, expect, it } from 'vitest'
import {
  mergeStatus,
  parseJob,
  parseOperation,
  parsePending,
  parsePlan,
  parseStatus,
  parseLogs,
  type Status
} from './controller'
const id = 'a'.repeat(64),
  key = 'b'.repeat(64)
const op = {
  kind: 'create' as const,
  project: 'shop',
  domain: 'api.shop.test',
  profile: '1gb'
}
const job = {
  id,
  request_key: key,
  operation: op,
  stage: 'applying' as const,
  sequence: 5,
  updated_at: 123,
  changed: true,
  backup: null,
  error: null
}
const status: Status = {
  receipt: null,
  enabled: true,
  projects: [],
  jobs: [job],
  max_projects: 32,
  ram_mb: 8192,
  free_bytes: 10000000
}
describe('host management boundaries', () => {
  it('rejects non-finite resource counters, malformed jobs and path selections', () => {
    for (const value of [NaN, Infinity, -1, 0.1, Number.MAX_SAFE_INTEGER + 1])
      expect(() => parseStatus({ ...status, free_bytes: value })).toThrow()
    for (const value of [
      null,
      { ...job, request_key: '../x' },
      { ...job, sequence: 0 },
      { ...job, changed: 'true' }
    ])
      expect(() => parseJob(value)).toThrow()
    for (const backup of ['../x.dump', '/x.dump', '--x.dump', 'x\\y.dump'])
      expect(() =>
        parseOperation({ kind: 'restore', project: 'shop', backup })
      ).toThrow()
    for (const domain of [
      'javascript:alert(1)',
      'https://api.shop.test',
      'api.test/path',
      'api.test\nadmin',
      '*.test'
    ])
      expect(() => parseOperation({ ...op, domain })).toThrow()
  })
  it('rejects unsafe project-panel link destinations', () => {
    const project = {
      name: 'shop',
      domain: 'api.shop.test',
      version: '0.4.0',
      app: 'healthy',
      database: 'healthy',
      backups: []
    }
    for (const domain of [
      'evil.test@other.test',
      'https://api.shop.test',
      'api.test/path',
      'api.test#fragment'
    ]) {
      expect(() =>
        parseStatus({ ...status, projects: [{ ...project, domain }] })
      ).toThrow()
    }
  })
  it('does not coerce invalid profile values from the wire', () => {
    for (const profile of [['1gb'], { value: '1gb' }, null, NaN, '16gb']) {
      expect(() => parseOperation({ ...op, profile })).toThrow()
    }
  })
  it('binds confirmation and destructive warnings to the exact operation', () => {
    const plan = {
      id,
      operation: op,
      confirmation: 'CREATE shop',
      expires_at: 100,
      version: '0.4.0',
      image_digest: key,
      blockers: [],
      ram_mb: 1024,
      free_bytes: 123,
      project_count: 1,
      destructive: false
    }
    expect(parsePlan(plan).operation).toEqual(op)
    expect(() =>
      parsePlan({ ...plan, confirmation: 'CREATE another' })
    ).toThrow()
    expect(() =>
      parsePlan({
        ...plan,
        operation: { kind: 'remove', project: 'shop' },
        confirmation: 'REMOVE shop'
      })
    ).toThrow()
  })
  it('keeps progress monotonic and retains accepted work across stale polls', () => {
    expect(
      mergeStatus(status, {
        ...status,
        jobs: [{ ...job, sequence: 2, stage: 'validating' }]
      }).jobs[0]
    ).toEqual(job)
    expect(mergeStatus(status, { ...status, jobs: [] }).jobs[0]).toEqual(job)
  })
  it('preserves only the identifiers required to retry the same request', () => {
    const pending = parsePending({
      plan: id,
      key,
      operation: op,
      confirmation: 'CREATE shop',
      password: 'must-not-be-persisted'
    })
    expect(pending).toEqual({
      plan: id,
      key,
      operation: op,
      confirmation: 'CREATE shop'
    })
    expect(
      parsePending({
        plan: id,
        key,
        operation: op,
        confirmation: 'REMOVE shop'
      })
    ).toBeNull()
    expect(parsePending(null)).toBeNull()
  })
  it('rejects free-form log messages and limits snapshots', () => {
    expect(() =>
      parseLogs([{ timestamp: '', level: '', target: '', message: 'secret' }])
    ).toThrow()
    expect(() =>
      parseLogs(
        Array.from({ length: 101 }, () => ({
          timestamp: '',
          level: '',
          target: '',
          message: 'event_metadata_only'
        }))
      )
    ).toThrow()
  })
})
