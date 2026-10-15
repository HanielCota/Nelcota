// Native k6 runner: the backend and benchmark runner require no Node runtime.
import http from 'k6/http';
import execution from 'k6/execution';
import { check } from 'k6';
import { Trend, Rate, Counter } from 'k6/metrics';

const fixture = JSON.parse(__ENV.FIXTURE);
const base = fixture.base;
const kind = __ENV.WORKLOAD || 'mixed';
const vus = Number(__ENV.VUS || 16);
const rate = Number(__ENV.RATE || 0);
if (!Number.isSafeInteger(vus) || vus < 1 || vus > 128 || !Number.isSafeInteger(rate) || rate < 0 || rate > 10000) {
  throw new Error('VUS/RATE are outside campaign budgets');
}
const duration = __ENV.DURATION || '30s';
export const options = {
  scenarios: { traffic: rate > 0 ? {
    executor: 'constant-arrival-rate', rate, timeUnit: '1s', duration,
    preAllocatedVUs: vus, maxVUs: 128,
  } : { executor: 'constant-vus', vus, duration } },
  systemTags: ['status', 'method', 'group', 'check', 'error_code', 'scenario'],
  tags: { run_id: __ENV.RUN_ID || 'standalone' },
  summaryTrendStats: ['avg', 'p(50)', 'p(95)', 'p(99)', 'max'],
  // Keep bodies only for credential rotation and explicit RLS invariants.
  discardResponseBodies: true,
  thresholds: { unexpected: [{ threshold: 'rate==0', abortOnFail: true, delayAbortEval: '1s' }] },
};
const unexpected = new Rate('unexpected');
const successfulOperations = new Counter('successful_operations');
const successfulDuration = new Trend('successful_duration', true);
const assertionsPassed = new Counter('assertions_passed');
const assertionsFailed = new Counter('assertions_failed');
const expectedRejections = new Counter('expected_rejections');
const overloadRejections = new Counter('overload_rejections');
function fail() {
  assertionsFailed.add(1);
  unexpected.add(true);
  execution.test.abort('Stress correctness invariant failed');
}
const timings = Object.fromEntries(['read','rls','panel','storage','login','refresh','write'].map(name => [name, new Trend(`latency_${name}`, true)]));
let refreshToken;
let accessToken;
let accessExpiresAt = 0;

function request(name, method, path, { body = null, token, cookie } = {}) {
  const headers = { 'Content-Type': 'application/json' };
  if (token) { headers.Authorization = `Bearer ${token}`; }
  if (cookie) { headers.Cookie = cookie; }
  const responseType = ['rls','login','refresh'].includes(name) ? 'text' : 'none';
  const response = http.request(method, base + path, body, { headers, responseType, timeout: '12s', tags: { workload: name } });
  const ok = response.status >= 200 && response.status < 300;
  if (ok) {
    timings[name].add(response.timings.duration);
    successfulOperations.add(1, { workload: name });
    successfulDuration.add(response.timings.duration);
  }
  const bounded = __ENV.OVERLOAD === 'true' && [429, 503, 504].includes(response.status);
  overloadRejections.add(0);
  if (bounded) { overloadRejections.add(1); }
  assertionsFailed.add(0);
  if (!ok && !bounded) { fail(); }
  assertionsPassed.add(1);
  unexpected.add(false);
  check(response, { [`${name}: bounded response`]: () => ok || bounded });
  return response;
}

function read() {
  request('read','GET',`/rest/v1/bench_products?stock=eq.${__ITER % 50}&order=id&limit=20`);
}
function rls() {
  if (!ensureAccess()) { return; }
  const result = request('rls','GET','/rest/v1/bench_private?order=id&limit=20',{token:accessToken});
  if (result.status === 200) {
    let rows;
    try { rows = result.json(); } catch { fail(); return; }
    const isolated = Array.isArray(rows) && rows.length > 0 && rows.every(row => row !== null && typeof row === 'object' && row.user_id === fixture.user);
    if (!isolated) { fail(); }
    assertionsPassed.add(1);
    check(result, { 'RLS preserves ownership': () => isolated });
  }
}
function panel() {
  request('panel','GET','/admin/api/tables/bench_products?limit=20',{cookie:fixture.cookie});
}
function storage() {
  const operation = __ITER % 3;
  if (operation === 0) {
    request('storage','POST','/storage/v1/object/list/bench',{body:JSON.stringify({prefix:'',limit:20,offset:0}),token:fixture.service});
    return;
  }
  if (operation === 1) {
    request('storage','GET','/storage/v1/object/public/bench/fixture.txt');
    return;
  }
  request('storage','PUT',`/storage/v1/object/bench/probe-${__VU}.txt`,{body:'benchmark payload',token:fixture.service});
}
function installSession(result) {
  if (result.status !== 200) { return false; }
  let value;
  try { value = result.json(); } catch { fail(); return false; }
  if (!value || typeof value.access_token !== 'string' || !value.access_token || typeof value.refresh_token !== 'string' || !value.refresh_token || !Number.isFinite(value.expires_in) || value.expires_in <= 0) {
    fail();
    return false;
  }
  accessToken = value.access_token;
  refreshToken = value.refresh_token;
  accessExpiresAt = Date.now() + value.expires_in * 1000;
  return true;
}
function login() {
  return installSession(request('login','POST','/auth/v1/token?grant_type=password',{body:JSON.stringify({email:fixture.email,password:fixture.password})}));
}
function refresh() {
  if (!refreshToken) { return login(); }
  return installSession(request('refresh','POST','/auth/v1/token?grant_type=refresh_token',{body:JSON.stringify({refresh_token:refreshToken})}));
}
function ensureAccess() {
  if (accessToken && Date.now() < accessExpiresAt - 30000) { return true; }
  if (refreshToken) { return refresh(); }
  return login();
}
function write() {
  request('write','PATCH',`/rest/v1/bench_products?id=eq.${(__ITER % 1000) + 1}`,{body:JSON.stringify({name:`product ${__ITER % 1000}`}),token:fixture.service});
}
const operations = {read,rls,panel,storage,login,refresh,write};
function rejectInvalidCredential() {
  if ((__ITER + __VU) % 1000 !== 0) { return; }
  const result = http.get(base + '/rest/v1/bench_private?limit=1', { headers: { Authorization: 'Bearer ' + fixture.invalid_access }, responseType: 'none', timeout: '5s', tags: { workload: 'security' } });
  if (![401, 403].includes(result.status)) { fail(); }
  assertionsPassed.add(1);
  expectedRejections.add(1);
}
export default function () {
  rejectInvalidCredential();
  if (kind !== 'mixed') {
    operations[kind]();
    return;
  }
  // Deterministic per-VU cycle: 50% read, 20% RLS, 10% panel, 5% storage,
  // 5% hashing, 5% rotation and 5% signed webhook-producing writes.
  const step = (__ITER + __VU) % 20;
  if (step < 10) { read(); return; }
  if (step < 14) { rls(); return; }
  if (step < 16) { panel(); return; }
  if (step === 16) { storage(); return; }
  if (step === 17) { login(); return; }
  if (step === 18) { refresh(); return; }
  write();
}
