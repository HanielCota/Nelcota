// Native k6 runner: the backend and benchmark runner require no Node runtime.
import http from 'k6/http';
import execution from 'k6/execution';
import { check, sleep } from 'k6';
import { Trend, Rate, Counter } from 'k6/metrics';

const configuration = JSON.parse(__ENV.FIXTURE);
const projects = configuration.projects ?? [configuration];
let fixture = projects[0];
const objectBytes=fixture.object_bytes ?? 17;
if (!Number.isSafeInteger(objectBytes) || objectBytes<1 || objectBytes>1048576) { throw new Error('Object payload exceeds its bounded fixture range'); }
function payload() {
  if (fixture.marker) { return fixture.marker+' payload'; }
  if (objectBytes===17) { return 'benchmark payload'; }
  return 'x'.repeat(objectBytes);
}
let base = fixture.base;
let projectIndex = 0;
const fleet = __ENV.FLEET === 'true';
const sessionCanary = __ENV.SESSION_CANARY === 'true';
function scenarios() {
  if (fleet) {
    const rates = (__ENV.PROJECT_RATES || '').split(',').map(Number);
    if (projects.length < 1 || projects.length > 10 || rates.length !== projects.length || rates.some(value => !Number.isSafeInteger(value) || value < 1) || rates.reduce((a,b) => a+b,0) > 10000) { throw new Error('Invalid bounded fleet rates'); }
    return Object.fromEntries(projects.map((project,index) => ['project-'+index, { executor:'constant-arrival-rate', rate:rates[index], timeUnit:'1s', duration, preAllocatedVUs:2, maxVUs:Math.floor(128/projects.length), exec:'project'+index }]));
  }
  if (rate > 0) {
    if (sessionCanary) { return { traffic:{ executor:'constant-arrival-rate',rate,timeUnit:'1s',duration,preAllocatedVUs:vus,maxVUs:127 }, 'session-canary':{executor:'constant-vus',vus:1,duration,exec:'sessionProbe'} }; }
    return { traffic:{ executor:'constant-arrival-rate',rate,timeUnit:'1s',duration,preAllocatedVUs:vus,maxVUs:128 } };
  }
  return { traffic:{ executor:'constant-vus',vus,duration } };
}
const kind = __ENV.WORKLOAD || 'mixed';
const vus = Number(__ENV.VUS || 16);
const rate = Number(__ENV.RATE || 0);
if (!Number.isSafeInteger(vus) || vus < 1 || vus > 128 || !Number.isSafeInteger(rate) || rate < 0 || rate > 10000) {
  throw new Error('VUS/RATE are outside campaign budgets');
}
const duration = __ENV.DURATION || '30s';
export const options = {
  scenarios: scenarios(),
  systemTags: ['status', 'method', 'group', 'check', 'error_code', 'scenario'],
  tags: { run_id: __ENV.RUN_ID || 'standalone', project:'single' },
  summaryTrendStats: ['avg', 'p(50)', 'p(95)', 'p(99)', 'max'],
  // Keep bodies only for credential rotation and explicit RLS invariants.
  discardResponseBodies: true,
  thresholds: { unexpected: [{ threshold: 'rate==0', abortOnFail: true, delayAbortEval: '1s' }] },
};
const unexpected = new Rate('unexpected');
const successfulOperations = new Counter('successful_operations');
const accessRenewals=new Counter('access_renewals');
const successfulDuration = new Trend('successful_duration', true);
const assertionsPassed = new Counter('assertions_passed');
const assertionsFailed = new Counter('assertions_failed');
const expectedRejections = new Counter('expected_rejections');
const overloadRejections = new Counter('overload_rejections');
const invariantFailures = new Counter('invariant_failures');
function fail(reason = 'assertion', workload = 'invariant', status = 0) {
  assertionsFailed.add(1);
  unexpected.add(true);
  // Fixed operation labels and numeric status only: no URLs, bodies or tokens.
  invariantFailures.add(1, { reason, workload, status: String(status) });
  execution.test.abort(`Stress invariant failed: ${reason}; workload=${workload}; status=${status}`);
}
const timings = Object.fromEntries(['read','rls','panel','storage','login','refresh','write'].map(name => [name, new Trend(`latency_${name}`, true)]));
let refreshToken;
let accessToken;
let accessExpiresAt = 0;

function request(name, method, path, { body = null, token, cookie } = {}) {
  const headers = { 'Content-Type': 'application/json' };
  if (token) { headers.Authorization = `Bearer ${token}`; }
  if (cookie) { headers.Cookie = cookie; }
  let responseType = 'none';
  if (['rls','login','refresh'].includes(name) || (fixture.marker && ['read','storage'].includes(name))) { responseType = 'text'; }
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
  if (!ok && !bounded) { fail('unexpected_http_status', name, response.status); }
  assertionsPassed.add(1);
  unexpected.add(false);
  check(response, { [`${name}: bounded response`]: () => ok || bounded });
  return response;
}

function read() {
  const result = request('read','GET',`/rest/v1/bench_products?stock=eq.${__ITER % 50}&order=id&limit=20`);
  if (fixture.marker && result.status === 200) {
    let rows;
    try { rows = result.json(); } catch { fail('malformed_read', 'read', result.status); return; }
    if (!Array.isArray(rows) || !rows.length || !rows.every(row => row && typeof row.name === 'string' && row.name.startsWith(fixture.marker))) { fail('project_read_boundary', 'read', result.status); }
    assertionsPassed.add(1);
  }
}
function rls() {
  if (!ensureAccess()) { return; }
  const result = request('rls','GET','/rest/v1/bench_private?order=id&limit=20',{token:accessToken});
  if (result.status === 200) {
    let rows;
    try { rows = result.json(); } catch { fail('malformed_rls', 'rls', result.status); return; }
    const isolated = Array.isArray(rows) && rows.length > 0 && rows.every(row => row !== null && typeof row === 'object' && row.user_id === fixture.user && (!fixture.marker || row.body === fixture.label));
    if (!isolated) { fail('rls_boundary', 'rls', result.status); }
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
    const result = request('storage','GET','/storage/v1/object/public/bench/fixture.txt');
    if (fixture.marker && result.status === 200 && result.body !== JSON.stringify(fixture.marker+' payload')) { fail('object_bytes', 'storage', result.status); }
    return;
  }
  request('storage','PUT',`/storage/v1/object/bench/probe-${__VU}.txt`,{body:payload(),token:fixture.service});
}
function installSession(result) {
  if (result.status !== 200) { return false; }
  let value;
  try { value = result.json(); } catch { fail('malformed_session', 'auth', result.status); return false; }
  if (!value || typeof value.access_token !== 'string' || !value.access_token || typeof value.refresh_token !== 'string' || !value.refresh_token || !Number.isFinite(value.expires_in) || value.expires_in <= 0) {
    fail('invalid_session_contract', 'auth', result.status);
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
  if (refreshToken) {
    const renewed=refresh();
    if (renewed && sessionCanary && execution.scenario.name==='session-canary') { accessRenewals.add(1); }
    return renewed;
  }
  return login();
}
function write() {
  const name = fixture.marker ? `${fixture.marker}${__ITER % 1000}` : `product ${__ITER % 1000}`;
  request('write','PATCH',`/rest/v1/bench_products?id=eq.${(__ITER % 1000) + 1}`,{body:JSON.stringify({name}),token:fixture.service});
}
const operations = {read,rls,panel,storage,login,refresh,write};
function rejectInvalidCredential() {
  if ((__ITER + __VU) % (fleet ? 10 : 1000) !== 0) { return; }
  const result = http.get(base + '/rest/v1/bench_private?limit=1', { headers: { Authorization: 'Bearer ' + fixture.invalid_access }, responseType: 'none', timeout: '5s', tags: { workload: 'security' } });
  if (result.status !== 401) { fail('invalid_credential_response', 'security', result.status); }
  assertionsPassed.add(1);
  expectedRejections.add(1);
}
export default function runCurrent() {
  rejectInvalidCredential();
  if (fleet && (__ITER + __VU) % 10 === 0 && projects.length > 1) {
    const foreign = projects[(projectIndex+1)%projects.length];
    const result = http.get(base+'/rest/v1/bench_products?limit=1',{headers:{Authorization:'Bearer '+foreign.service},responseType:'none',timeout:'5s',tags:{workload:'cross-project'}});
    if (result.status !== 401) { fail('foreign_credential_response', 'cross-project', result.status); }
    assertionsPassed.add(1); expectedRejections.add(1);
  }
  if (fleet && projectIndex === 0 && __ENV.HOT_WORKLOAD && __ENV.HOT_WORKLOAD !== 'mixed') { operations[__ENV.HOT_WORKLOAD](); return; }
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

function selectProject(index) {
  if (!projects[index]) { fail(); return; }
  if (projectIndex !== index) {
    projectIndex = index; fixture = projects[index]; base = fixture.base;
    refreshToken = undefined; accessToken = undefined; accessExpiresAt = 0;
  }
  execution.vu.metrics.tags.project = fixture.label;
}
export function project0() { selectProject(0); runCurrent(); }
export function project1() { selectProject(1); runCurrent(); }
export function project2() { selectProject(2); runCurrent(); }
export function project3() { selectProject(3); runCurrent(); }
export function project4() { selectProject(4); runCurrent(); }
export function project5() { selectProject(5); runCurrent(); }
export function project6() { selectProject(6); runCurrent(); }
export function project7() { selectProject(7); runCurrent(); }
export function project8() { selectProject(8); runCurrent(); }
export function project9() { selectProject(9); runCurrent(); }

export function sessionProbe() { rejectInvalidCredential(); rls(); sleep(1); }
