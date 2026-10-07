// Leitura simples: filtro indexado + ordem + página de 20 linhas.
import http from 'k6/http';
import { check } from 'k6';

const BASE = __ENV.BASE_URL || 'http://app:8000';

export const options = {
  vus: Number(__ENV.VUS || 32),
  duration: __ENV.DURATION || '30s',
  insecureSkipTLSVerify: true,
  summaryTrendStats: ['avg', 'p(50)', 'p(95)', 'p(99)', 'max'],
  thresholds: { http_req_failed: ['rate<0.001'] },
};

export default function () {
  const estoque = Math.floor(Math.random() * 50);
  const res = http.get(`${BASE}/rest/v1/bench_produtos?estoque=eq.${estoque}&order=id&limit=20`);
  check(res, { 'status 200': (r) => r.status === 200 });
}
