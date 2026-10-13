// Native k6 runner: the backend and benchmark runner require no Node runtime.
import http from 'k6/http';
import { check } from 'k6';
import { Trend, Rate } from 'k6/metrics';

const fixture = JSON.parse(open(__ENV.FIXTURE));
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
  summaryTrendStats: ['avg', 'p(50)', 'p(95)', 'p(99)', 'max'],
  // Keep bodies only for credential rotation and explicit RLS invariants.
  discardResponseBodies: true,
  thresholds: { unexpected: ['rate==0'] },
};
const unexpected = new Rate('unexpected');
const timings = Object.fromEntries(['read','rls','panel','storage','login','refresh','write'].map(name => [name, new Trend(`latency_${name}`, true)]));
let refreshToken;

function request(name, method, path, { body = null, token, cookie } = {}) {
  const headers = { 'Content-Type': 'application/json' };
  if (token) { headers.Authorization = `Bearer ${token}`; }
  if (cookie) { headers.Cookie = cookie; }
  const responseType = ['rls','login','refresh'].includes(name) ? 'text' : 'none';
  const response = http.request(method, base + path, body, { headers, responseType, timeout: '12s', tags: { workload: name } });
  timings[name].add(response.timings.duration);
  const ok = response.status >= 200 && response.status < 300;
  const bounded = __ENV.OVERLOAD === 'true' && [429, 503, 504].includes(response.status);
  unexpected.add(!ok && !bounded);
  check(response, { [`${name}: bounded response`]: () => ok || bounded });
  return response;
}

function read() {
  request('read','GET',`/rest/v1/bench_products?stock=eq.${__ITER % 50}&order=id&limit=20`);
}
function rls() {
  const result = request('rls','GET','/rest/v1/bench_private?order=id&limit=20',{token:fixture.access});
  if (result.status === 200) {
    const rows = result.json();
    const isolated = rows.length > 0 && rows.every(row => row.user_id === fixture.user);
    unexpected.add(!isolated);
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
function login() {
  const result = request('login','POST','/auth/v1/token?grant_type=password',{body:JSON.stringify({email:fixture.email,password:fixture.password})});
  if (result.status === 200) { refreshToken = result.json('refresh_token'); }
}
function refresh() {
  if (!refreshToken) { login(); }
  if (!refreshToken) { return; }
  const result = request('refresh','POST','/auth/v1/token?grant_type=refresh_token',{body:JSON.stringify({refresh_token:refreshToken})});
  if (result.status === 200) { refreshToken = result.json('refresh_token'); }
}
function write() {
  request('write','PATCH',`/rest/v1/bench_products?id=eq.${(__ITER % 1000) + 1}`,{body:JSON.stringify({name:`product ${__ITER % 1000}`}),token:fixture.service});
}
const operations = {read,rls,panel,storage,login,refresh,write};
export default function () {
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
