# Benchmark

The current supported measurement workflow is the release campaign below, using
[k6](https://k6.io), Docker and dedicated Linux CPU slots. Historical results
describe their original fixtures, not current VPS capacity.

The older read-only fixture queried:
`GET /rest/v1/bench_products?stock=eq.N&order=id&limit=20` over 100k rows
(indexed filter, page of 20), as `anon`.

The legacy `bench/run.sh` assumes the former single-project Compose layout and
`nelcota_web` network. Current hosts use per-project Compose files, `nelcota_edge`
and project-specific app aliases, so that runner needs a separate tooling fix.
Do not use it as current validation. Use `campaign.py` below instead.

## Measured result (2026-10-06)

The later release campaign below is a separate fixture and operating envelope;
do not compare its resource-limited workload numbers to this laptop result.

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

These laptop container readings do not establish available RAM on a VPS.
Measure process/container usage, PostgreSQL buffers, page cache and OS overhead
under your own workload before sizing a deployment.

### Automated tests

`cargo test --release --test api performance -- --nocapture` (300 sequential
reads over 20k rows, no concurrency): ~1.5 ms per request on average, Docker
networking included.

## Release campaign

On Linux with Docker, at least four affinity slots and an effective CPU quota
of 3.5 CPUs for disk or 4 CPUs for S3:

```sh
cargo build --locked --release -p nelcota-server
nice -n19 ionice -c3 python3 bench/campaign.py --binary target/release/nelcota --output target/campaign
```

The campaign owns its disposable PostgreSQL, runtime-generated credentials and
disk files. App, PostgreSQL and the native k6 generator use separate CPU affinity
slots with CPU/memory quotas (app 1 CPU/128 MiB, PostgreSQL 1 CPU/384 MiB,
generator 1 CPU/512 MiB with Go's memory target at 448 MiB, Prometheus
0.25 CPU/512 MiB, request pool 8,
database connections 40). Keep unrelated
work off those CPU slots. The app runs without root privileges. Reports contain
metrics, plans and redacted logs; credential fixtures are deleted on teardown.

PostgreSQL data uses an owned disk volume, removed at teardown. WAL targets
64 MiB and the fixture's monitored disk budget is 512 MiB. Database files do not
use tmpfs. Local object files must also reside on a real disk filesystem:
counting growing data/WAL against a small RAM quota would turn the
soak into a memory-filesystem experiment. k6 containers are named and removed
on errors/timeouts as well as normal completion.

Three steady runs per scenario measure anonymous indexed reads, authenticated
RLS with hidden rows, panel pages, actual disk listing/download/overwrite,
password login, rotation and mixed traffic. Fresh and analyzed statistics have
separate runs/plans. Mixed traffic has 50% reads, 20% RLS, 10% panel, 5% storage,
5% hashing, 5% rotation and 5% signed webhook-producing writes. Memory/RSS,
CPU, connections, idle transactions, locks and queue occupancy are sampled.

The stress cycle uses 16/32/64/128 VUs, recovers at four VUs and restarts the
server. The default soak lasts **1,800 real seconds**, at 80% of independently
qualified mixed iterations/second. Five-minute open-loop probes qualify rates
against G05 SLOs, useful work and delivered arrivals. Rejected requests do not
enter successful-response latency. Failed capacity probes remain in the evidence. The queue must also drain within 30 seconds after traffic
stops. A failed response, RLS check, signature or resource invariant fails the
candidate campaign. Database lock/unavailability, slow export cancellation,
multipart/provider failures and lease recovery additionally have dedicated
real-service Rust regressions; see the execution report.

For an isolated S3 workload with actual uploads/downloads on RustFS:

```sh
python3 bench/campaign.py --binary target/release/nelcota --output target/s3-campaign --backend s3 --workloads storage --soak-seconds 0 --no-stress
```

RustFS adds a 0.5-CPU/512-MiB fixture on a fourth CPU affinity slot; S3
campaigns require four effective CPUs including the observer/OS reserve. The small S3 campaign does not claim a
30-minute S3 soak or production cloud/network capacity. Long campaigns belong
on a dedicated host. `.github/workflows/stress.yml` provides a manual job for a
self-hosted Linux runner labeled `nelcota-benchmark`, keeping this workload
outside ordinary PR checks.

Save a release baseline binary before refactoring, then compare equivalent runs:

```sh
python3 bench/compare.py target/baseline target/candidate --output target/comparison.json
```

Median p95 may worsen at most 10%; throughput may decrease at most 10%.
Unexpected failures are recorded separately. A failed baseline sample cannot
certify a candidate. The comparator requires three valid samples on both
sides, finite positive latency/throughput and matching resource, image,
dataset, backend and nominal-duration evidence. Incomparable reports fail.
Use an empty output directory for every campaign; existing reports are never
appended to or overwritten. Open-loop delivery must reach 99% of the requested
iterations with at most 1% dropped arrivals. Post-load byte checks verify the
bounded fixture objects, and diagnostics cannot bypass teardown. `--allow-baseline-failures`
collects failures/restarts of an old binary and explicitly reports them;
never use that option to certify a candidate. `--workloads` and `--steady`
select smaller investigations, while `--soak-seconds 0` disables the long soak.
TLS verification in the original read runner is enabled by default; only a
controlled development test should set `INSECURE_TLS=true`.

## G05 endurance and independent projects

Freeze the harness and keep a unique run ID. Never overwrite a failed run:

```sh
python3 bench/launch.py --run-id disk-pass-1 --binary target/release/nelcota \
  --soak-seconds 14400 --fixture-disk-mib 2048 --capacity --spikes
python3 bench/launch.py --run-id s3-pass-1 --binary target/release/nelcota \
  --backend s3 --soak-seconds 14400 --fixture-disk-mib 2048 --capacity --spikes
python3 bench/launch.py --run-id host-ten-pass-1 --kind host \
  --binary target/release/nelcota --projects 10 --seconds 300 --rate 8
```

Run these sequentially on this profile. The single-project watchdog has a
six-hour deadline, bounded preparation, 5-second resource samples and a maximum
30-second observation gap. Host reserve floors are 10% available memory and
15% free bytes/inodes. Preflight additionally reserves room for declared data
growth, container roots and telemetry. A breached budget stops traffic and
removes only explicitly owned resources. Keep an external deployment observer
for power-loss acceptance; a local watchdog does not survive host loss.

Endurance uses bounded native Prometheus histograms. k6's raw end-of-test trend
collector is disabled; the script still aborts immediately on correctness
violations. Output includes `ownership.json`, `metadata.json`, frozen source
hashes, `traffic-ledger.jsonl`, `resources.jsonl`, capacity probes, retention
cohorts, event reconciliation, cleanup and a final `completion.json`. Missing
or failed evidence cannot certify a campaign. Duration counts completed,
positive-traffic intervals; preparation and waiting are excluded.

The fixture explicitly clears terminal webhook history through the documented
admin API at 5,000 retained records, while an independent commit ledger keeps
event identities. This does not change the product's 100,000-history admission
limit; testing that limit without maintenance is a separate scenario. Two
safely aged auth/event cohorts exercise the real pruning loops during long
endurance. User access credentials rotate through supported login/refresh.

The host runner creates 1, 5 or 10 independent app/PostgreSQL clusters with
distinct keys, passwords and stores, intentionally reused user IDs and marked
database/file bytes. Apps share an aggregate 1-CPU budget and databases another
1 CPU; each app has 128 MiB and each database 256 MiB. k6 and telemetry retain
their own slots. Three balanced samples establish the median. Noisy-neighbor
phases leave healthy arrivals unchanged and send 80% of offered work to project
zero, checking absolute SLOs and the 90%-throughput/120%-p95 fairness gates.
`--seconds 30` is a functional smoke and does not certify five-minute steps.
This 1,000-row topology measures local application/database isolation; it does
not certify the host controller, Caddy/public TLS, larger datasets or VPS sizing.

The manual workflow supports separate disk/S3 four-hour endurance jobs with a
seven-hour job budget including the build. The internal campaign limit remains
six hours. Ordinary pull-request jobs do not launch endurance.

Campaign credentials are generated at runtime and passed to native k6 through
the Docker child environment. The generator command contains the environment
variable name, without its value; no plaintext credential fixture file is
written or mounted. Treat Docker daemon access as privileged, as for the
application and database containers' existing runtime configuration.

Resource evidence requires at least two successful samples and a maximum
30-second gap, including the beginning/end of observed traffic. Sampler
exceptions invalidate the campaign unless the observation crossed its explicit
server-restart window; cross-restart observations are discarded. The sampler
must finish before the final evidence gate, so a late failure cannot be hidden.

## Troubleshooting and evidence

Use a fresh output directory and inspect failed/skipped resource gates. A
baseline failure cannot certify a candidate. Record binary hashes, CPU/memory
budgets, provider/runtime versions and dataset before comparing runs. The legacy
laptop table above has no current source/artifact identity and is historical.
See [execution evidence](../docs/goals/execution-report.md) for saved campaign
reports and practical limits; [contributing](../CONTRIBUTING.md) covers test isolation.


## G05 extended fixture profiles

The full fixture creates the 10,000 empty objects physically, with matching
size/hash metadata. S3 preparation has eight workers, at most 32 queued objects
and a 15-minute deadline. PostgreSQL and object data have separate monitored disk
budgets. S3 uses a labeled named volume. Docker operations verify ownership and
use immutable container IDs; a foreign volume is refused before mounting.
A critical stop can signal a verified generator through pidfd independently
of the Docker API.

Long runs hold one continuous RLS session and renew its token near actual expiry.
Its separate scenario cannot mask dropped nominal arrivals; combined admission
remains 128 VUs. Two aged cohorts exercise auth, webhook and physical-object GC.
PID/container/StartedAt checks preserve process continuity. Per-container cgroup
counters include PostgreSQL child OOMs; controller counters describe the controller.
Component cgroup observations also retain memory current/peak, anonymous/file
cache counters, CPU quota/throttling and task gauges. The manifest identifies
this observation protocol so comparisons cannot silently mix instrumentation.
PostgreSQL observations include table/vacuum, WAL, checkpointer, I/O and blocker
statistics. Lock waits and slow statements remain in private fixture logs.
On an unexpected generator failure, safe host reserves permit a private
64-MiB-limited database snapshot with a 30-second deadline. Critical resource
stops skip this extra work; failed/partial diagnostics remain explicit.

`--wal-mib 64|128|256|512` names a separate database resource profile; its disk
budget must cover at least three WAL budgets. `--soak-fraction 0.7|0.75|0.8`
chooses the declared share of independently qualified capacity. Defaults remain
64 MiB and 0.8. Changing either value requires fresh comparable measurements.
Owned containers cap their local Docker logs at two 8-MiB files. Spike campaigns
record 15 seconds without offered traffic after each recovery, and all campaigns
record 30 seconds of final rest. Queue drain, connections, locks, process owners
and component cgroups are retained in `resource-plateaus.json`. Rest is excluded
from effective load duration; interpreting live-owner growth still requires the
full timeline and application evidence.

Use `--rows 1000000` for the separate million-row investigation and
`--object-bytes 1048576` for 1-MiB transfers. These profiles need separate
baselines. `--metadata-only-seed` is explicitly a listing investigation and
cannot certify full physical-fixture integrity.

The fault runner checks atomic transfers, balances, a hot counter and committed
operation identities. Disconnected replies stay ambiguous until reconciliation;
no mutation is automatically retried. It covers deadlock, database/app restart,
tagged OOM, owned tmpfs byte/inode exhaustion and local live-writer snapshot
restore. The tmpfs fault does not certify physical disk endurance.
With `--s3-faults`, the runner also starts an owned real RustFS instance,
restarts and pauses it under database writers and storage probes, and requires
verified reads and new writes within the same recovery deadline. Its separate
CPU, memory, disk and ownership budgets are part of the manifest.

```sh
python3 bench/launch.py --run-id fault-smoke --kind fault --binary target/release/nelcota --rate 8
python3 bench/launch.py --run-id fault-pass-1 --kind fault --binary target/release/nelcota --capacity --s3-faults
python3 bench/host_compare.py target/stress-g05/host-before target/stress-g05/host-after --output target/stress-g05/host-comparison.json
python3 bench/evidence.py target/stress-g05/disk-pass-1 --output target/stress-public/disk-pass-1
```

Functional smoke has no capacity claim. Full fault mode holds four five-minute
arrival steps and uses 80% of the qualified lower bound. Host comparisons need
three completed five-minute samples with equivalent budgets, images, dataset
and rates. Per-operation p95 and useful-throughput limits remain 10%. Rejected
HTTP requests cannot mask lost useful work; a later failed campaign cannot
certify successful initial samples.
`--focus filesystem` or `--focus provider --s3-faults` isolates an investigation;
the manifest names the subset and cannot certify the complete fault matrix.
Filesystem setup copies bounded immutable bytes through `docker exec` into the
actual tmpfs mount, then verifies reads before exhaustion. Byte reservation
rejects a full device with `507 storage_full`; failed inode allocation uses the
existing `500 internal` contract. Other errors and loss of existing bytes fail.

Public bundles contain bounded structured outcomes and hashes. Raw logs,
credential fixtures, database dumps and free-form diagnostics stay private.
Failed/invalid evidence remains explicit; the exporter does not certify G05.
