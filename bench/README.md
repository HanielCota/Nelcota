# Benchmark

A simple read, reproducible with [k6](https://k6.io):
`GET /rest/v1/bench_products?stock=eq.N&order=id&limit=20` over 100k rows
(indexed filter, page of 20), as `anon`.

```sh
nelcota init --local --yes --profile 1gb && nelcota up
VUS=32 DURATION=30s ./bench/run.sh .
```

`run.sh` creates the data (`setup.sql`), runs k6 in a container on the compose
network pointing straight at the app (`http://app:8000`, without Caddy, to
measure the API) and shows memory usage. To measure with Caddy and TLS in the
path, use `BASE_URL=https://your-domain`.

## Measured result (2026-10-06)

Environment: a Windows 11 laptop + Docker Desktop (WSL2), 8 vCPUs, k6 on the
same machine. Postgres `1gb` profile and 1 GB VPS memory limits applied to the
containers (app 128 MB, Postgres 640 MB, Caddy 128 MB). **This is not a VPS**:
repeat on your target machine before drawing conclusions.

| Metric | Value |
|---|---|
| Throughput | **5,175 req/s** (32 VUs, 30 s) |
| Latency p50 / p95 / p99 | 4.9 ms / 12.6 ms / **29.6 ms** |
| Errors | 0 of 155,310 |

### Memory

| Container | Idle | Peak under load |
|---|---|---|
| app (nelcota) | 2.2 MiB | **5.2 MiB** |
| postgres (1gb profile) | 39 MiB | 69 MiB |
| caddy | 14 MiB | 13 MiB |
| **Total** | ~56 MiB | **~88 MiB** |

On a 1 GB VPS, more than 800 MB are left for the OS page cache, which is what
actually serves Postgres reads. The 128 MB of `shared_buffers` in the `1gb`
profile are only filled as pages are read.

### Automated tests

`cargo test --release --test api performance -- --nocapture` (300 sequential
reads over 20k rows, no concurrency): ~1.5 ms per request on average, Docker
networking included.
