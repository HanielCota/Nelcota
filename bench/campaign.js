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
if (!Array.isArray(projects) || projects.length<1 || projects.length>10) { throw new Error('Invalid bounded project fixture'); }
function scenarios() {
  if (fleet) {
    const rates = (__ENV.PROJECT_RATES || '').split(',').map(Number);
    if (projects.length < 1 || projects.length > 10 || rates.length !== projects.length || rates.some(value => !Number.isSafeInteger(value) || value < 1) || rates.reduce((a,b) => a+b,0) > 10000) { throw new Error('Invalid bounded fleet rates'); }
    return Object.fromEntries(projects.map((project,index) => ['project-'+index, { executor:'constant-arrival-rate', rate:rates[index], timeUnit:'1s', duration, preAllocatedVUs:Math.floor(128/projects.length), maxVUs:Math.floor(128/projects.length), exec:'project'+index }]));
  }
  if (rate > 0) {
    if (sessionCanary) { return { traffic:{ executor:'constant-arrival-rate',rate,timeUnit:'1s',duration,preAllocatedVUs:127,maxVUs:127 }, 'session-canary':{executor:'constant-vus',vus:1,duration,exec:'sessionProbe'} }; }
    return { traffic:{ executor:'constant-arrival-rate',rate,timeUnit:'1s',duration,preAllocatedVUs:128,maxVUs:128 } };
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
  setupTimeout: '120s',
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
const bootstrapSessions = new Counter('bootstrap_sessions');
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
const clientSessions = new Map();

export function setup() {
  if (!fleet && !['mixed','rls','refresh'].includes(kind) && !sessionCanary) { return null; }
  let slots=vus;
  if (fleet || rate>0 || sessionCanary) { slots=128; }
  const sessions=projects.map(() => Array(slots));
  const jobs=[];
  for (let slot=0;slot<slots;slot++) {
    for (let project=0;project<projects.length;project++) { jobs.push({project,slot}); }
  }
  for (let offset=0;offset<jobs.length;offset+=4) {
    const batch=jobs.slice(offset,offset+4);
    const responses=http.batch(batch.map(job => {
      const project=projects[job.project];
      return {method:'POST',url:project.base+'/auth/v1/token?grant_type=password',body:JSON.stringify({email:project.email,password:project.password}),params:{headers:{'Content-Type':'application/json'},responseType:'text',timeout:'12s',tags:{project:'bootstrap',workload:'client_bootstrap'}}};
    }));
    for (let index=0;index<batch.length;index++) {
      const job=batch[index];
      const response=responses[index];
      if (response.status!==200 || response.error_code!==0) { fail('client_bootstrap_status','bootstrap',response.status); }
      let value;
      try { value=response.json(); } catch { fail('client_bootstrap_json','bootstrap',response.status); }
      if (!value || typeof value.access_token!=='string' || !value.access_token || typeof value.refresh_token!=='string' || !value.refresh_token || !Number.isFinite(value.expires_in) || value.expires_in<=0 || value.user?.id!==projects[job.project].user) { fail('client_bootstrap_contract','bootstrap',response.status); }
      sessions[job.project][job.slot]=[value.access_token,value.refresh_token,Date.now()+value.expires_in*1000];
      bootstrapSessions.add(1,{project:'bootstrap'});
    }
  }
  return {sessions};
}

function hydrateSession(data) {
  if (!data?.sessions) { return; }
  let state=clientSessions.get(projectIndex);
  if (!state) {
    const prepared=data.sessions[projectIndex]?.[__VU-1];
    if (!Array.isArray(prepared) || prepared.length!==3 || typeof prepared[0]!=='string' || !prepared[0] || typeof prepared[1]!=='string' || !prepared[1] || !Number.isFinite(prepared[2]) || prepared[2]<=0) { fail('missing_bootstrap_session','bootstrap'); }
    state={access:prepared[0],refresh:prepared[1],expires:prepared[2]};
    clientSessions.set(projectIndex,state);
  }
  accessToken=state.access;refreshToken=state.refresh;accessExpiresAt=state.expires;
}

function request(name, method, path, { body = null, token, cookie } = {}) {
  const headers = { 'Content-Type': 'application/json' };
  if (token) { headers.Authorization = `Bearer ${token}`; }
  if (cookie) { headers.Cookie = cookie; }
  let responseType = 'none';
  if (['rls','login','refresh'].includes(name) || (name === 'storage' && method === 'GET') || (fixture.marker && name === 'read')) { responseType = 'text'; }
  const response = http.request(method, base + path, body, { headers, responseType, timeout: '12s', tags: { workload: name } });
  const successStatus = response.status >= 200 && response.status < 300;
  const ok = successStatus && response.error_code === 0;
  if (ok) {
    timings[name].add(response.timings.duration);
    successfulOperations.add(1, { workload: name });
    successfulDuration.add(response.timings.duration);
  }
  const bounded = __ENV.OVERLOAD === 'true' && [429, 503, 504].includes(response.status) && [0, 1000 + response.status].includes(response.error_code);
  overloadRejections.add(0);
  if (bounded) { overloadRejections.add(1); }
  assertionsFailed.add(0);
  if (successStatus && !ok) { fail('http_transport', name, response.status); }
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
    if (result.status >= 200 && result.status < 300 && result.status !== 200) { fail('object_status', 'storage', result.status); }
    if (result.status === 200) {
      let expected=payload();
      if (fixture.marker || objectBytes===17) { expected=JSON.stringify(expected); }
      if (result.body !== expected) { fail('object_bytes', 'storage', result.status); }
      assertionsPassed.add(1);
    }
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
  clientSessions.set(projectIndex,{access:accessToken,refresh:refreshToken,expires:accessExpiresAt});
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
  if (result.status !== 401 || ![0,1401].includes(result.error_code)) { fail('invalid_credential_response', 'security', result.status); }
  assertionsPassed.add(1);
  expectedRejections.add(1);
}
export default function runCurrent(data) {
  hydrateSession(data);
  rejectInvalidCredential();
  if (fleet && (__ITER + __VU) % 10 === 0 && projects.length > 1) {
    const foreign = projects[(projectIndex+1)%projects.length];
    const result = http.get(base+'/rest/v1/bench_products?limit=1',{headers:{Authorization:'Bearer '+foreign.service},responseType:'none',timeout:'5s',tags:{workload:'cross-project'}});
    if (result.status !== 401 || ![0,1401].includes(result.error_code)) { fail('foreign_credential_response', 'cross-project', result.status); }
    assertionsPassed.add(1); expectedRejections.add(1);
  }
  if (fleet && projectIndex === 0 && __ENV.HOT_WORKLOAD && __ENV.HOT_WORKLOAD !== 'mixed') { operations[__ENV.HOT_WORKLOAD](); return; }
  if (kind !== 'mixed') {
    operations[kind]();
    return;
  }
  // A global arrival cycle: 50% read, 20% RLS, 10% panel, 5% storage,
  // 5% hashing, 5% rotation and 5% signed webhook-producing writes.
  const step = execution.scenario.iterationInTest % 20;
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
export function project0(data) { selectProject(0); runCurrent(data); }
export function project1(data) { selectProject(1); runCurrent(data); }
export function project2(data) { selectProject(2); runCurrent(data); }
export function project3(data) { selectProject(3); runCurrent(data); }
export function project4(data) { selectProject(4); runCurrent(data); }
export function project5(data) { selectProject(5); runCurrent(data); }
export function project6(data) { selectProject(6); runCurrent(data); }
export function project7(data) { selectProject(7); runCurrent(data); }
export function project8(data) { selectProject(8); runCurrent(data); }
export function project9(data) { selectProject(9); runCurrent(data); }

export function sessionProbe(data) { hydrateSession(data); rejectInvalidCredential(); rls(); sleep(1); }
