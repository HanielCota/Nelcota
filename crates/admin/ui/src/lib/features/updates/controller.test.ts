import { describe, expect, it } from 'vitest'
import { controllerOrigin, parseJob, parsePlan, parseStatus, mergeStatus, type Job, type Status } from './controller'
const job: Job = { id: 'a'.repeat(64), project: 'shop', current: '0.4.0', target: '0.5.0', phase: 'validating', sequence: 6, updated_at: 123, committed: false, error: null }
const status: Status = { email: 'operator@example.test', projects: [{ name: 'shop', version: '0.4.0', update_available: true }], jobs: [job], release: { version: '0.5.0' }, enabled: true, checking: false, check_error: null }
describe('update capability responses', () => {
  it('rejects nullable, nonfinite, unsafe and unknown states', () => {
    for (const invalid of [null, {}, { ...job, phase: 'almost_done' }, { ...job, sequence: NaN }, { ...job, sequence: Infinity }, { ...job, sequence: Number.MAX_SAFE_INTEGER + 1 }, { ...job, id: '../state' }, { ...job, committed: 'false' }]) expect(() => parseJob(invalid)).toThrow()
    expect(() => parseStatus({ ...status, jobs: [null] })).toThrow()
    expect(() => parsePlan({ id: job.id, digest: job.id, project: 'shop', current: '0.4.0', target: '0.5.0', expires_at: Infinity, blockers: [] })).toThrow()
  })
  it('does not regress progress when a stale response arrives after reconnect', () => {
    const next = mergeStatus(status, { ...status, jobs: [{ ...job, sequence: 3, phase: 'capturing' }] })
    expect(next.jobs[0].phase).toBe('validating')
    expect(next.jobs[0].sequence).toBe(6)
    expect(mergeStatus(next, { ...status, jobs: [{ ...job, sequence: 8, phase: 'succeeded', committed: true }] }).jobs[0].phase).toBe('succeeded')
  })
  it('retains an accepted active job if a poll started before acceptance omits it', () => {
    expect(mergeStatus(status, { ...status, jobs: [] }).jobs[0]).toEqual(job)
    expect(mergeStatus(status, { ...status, email: 'another@example.test', jobs: [] }).jobs).toEqual([])
  })
  it('only exposes canonical HTTPS controller origins', () => {
    for (const value of [null, 'javascript:alert(1)', 'http://host.test', 'https://user:password@host.test', 'https://host.test/path', 'https://host.test?token=x']) expect(controllerOrigin(value)).toBeNull()
    expect(controllerOrigin('https://updates.example.test')).toBe('https://updates.example.test')
  })
})
