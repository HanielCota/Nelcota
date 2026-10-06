# Benchmark

Leitura simples, reproduzível com [k6](https://k6.io):
`GET /rest/v1/bench_produtos?estoque=eq.N&order=id&limit=20` sobre 100 mil
linhas (filtro indexado, página de 20), como `anon`.

```sh
nelcota init --local --yes --profile 1gb && nelcota up
VUS=32 DURATION=30s ./bench/run.sh .
```

O `run.sh` cria os dados (`setup.sql`), roda o k6 num container na rede do
compose apontando direto para o app (`http://app:8000`, sem o Caddy, para medir
a API) e mostra a memória. Para medir com o Caddy e o TLS no caminho, use
`BASE_URL=https://seu-dominio`.

## Resultado medido (2026-10-06)

Ambiente: notebook com Windows 11 + Docker Desktop (WSL2), 8 vCPUs, k6 na
mesma máquina. Perfil `1gb` do Postgres e limites de memória de VPS de 1 GB
aplicados aos containers (app 128 MB, Postgres 640 MB, Caddy 128 MB). **Não é
uma VPS**: repita na sua máquina-alvo antes de tirar conclusões.

| Métrica | Valor |
|---|---|
| Throughput | **5.175 req/s** (32 VUs, 30 s) |
| Latência p50 / p95 / p99 | 4,9 ms / 12,6 ms / **29,6 ms** |
| Erros | 0 de 155.310 |

### Memória

| Container | Em repouso | Pico sob carga |
|---|---|---|
| app (nelcota) | 2,2 MiB | **5,2 MiB** |
| postgres (perfil 1gb) | 39 MiB | 69 MiB |
| caddy | 14 MiB | 13 MiB |
| **Total** | ~56 MiB | **~88 MiB** |

Numa VPS de 1 GB, sobram mais de 800 MB para o cache de páginas do SO, que é
quem de fato serve as leituras do Postgres. Os 128 MB de `shared_buffers` do
perfil `1gb` só são ocupados conforme as páginas são lidas.

### Testes automatizados

`cargo test --release --test api desempenho -- --nocapture` (300 leituras
sequenciais sobre 20 mil linhas, sem concorrência): média de ~1,5 ms por
request, rede do Docker incluída.
