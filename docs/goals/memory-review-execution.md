# Focused memory lifecycle review

Reviewed on 2026-10-14, starting from `cc62f3ceaad07f7146b49489ad4f8c0b57fa4dc5`.
This follow-up extends G02/G04 with checks for background task ownership,
cancelled promise consumers, uploader cleanup and closed previews. Corrections
and their regression checks are recorded together with separate generated-asset
commits for the embedded panel.

## Findings and corrections

| Finding | Reproduction and consequence | Correction |
|---|---|---|
| Catalog reconciliation owns its discarded handle | A closed pool made refresh fail; dropping the last caller left the catalog and retry task alive. | The worker owns snapshot state, and the public handle owns cancellation. Drop aborts reconciliation, including an indefinite wait for a pool slot. |
| Catalog listeners have no lifetime owner | Starting/discarding 32 listeners left their reconnect tasks and catalog references alive. The separate PostgreSQL driver also had no cancellation guard. | `spawn_reload_listener` returns an owned `ReloadListener`; the server and test host retain it. Drop/shutdown cancels the loop, and a driver guard closes the connection on cancellation or error. |
| TypeScript SDK retains cancelled waiters | A pending shared operation kept all 1,000 cancelled callers' signals alive after garbage collection; settling it released them. Repetition could accumulate callers while storage/network work remained pending. | A shared observer set removes each cancelled caller immediately. One promise reaction observes completion/rejection, and an active caller still receives the accepted result. |
| Panel query readers retain cancelled consumers | With another reader still active, garbage collection found 1,000 cancelled signals retained by promise callbacks. | A detachable completion slot releases the reader's closure and abort listener at cancellation, while preserving the active reader and upstream request. |
| Closed upload conflict dialogs retain Files | With uploader listener cleanup in place, cancelling the dialog still kept the original File alive. | Closing the dialog clears the conflict array and captured destination. A replacement already in flight keeps its own captured inputs. |
| Uploader notification timers retain File callbacks | Chromium's heap path ran from a DOM timer through Uppy, its emitter and our upload-error callback to the File after `destroy()`. This is temporary retention by the dependency's timers. | Unregister our progress/error handlers before destroying Uppy so deferred dependency notifications cannot retain the File or progress consumer. |
| Closed previews retain decoded payloads | The dialog component remains mounted after close and kept its previous text/data URL. This is bounded retention of the last preview, rather than demonstrated unbounded growth. | Preview cleanup clears decoded media/text as well as aborting the read and revoking the object URL. |

### Compatibility and resource ownership

Consumers of the public Rust `spawn_reload_listener` helper must keep its returned
owner for the host's lifetime. Both repository callers were updated; the server
explicitly shuts it down, and the test host drops it with its other resources.
Dropping a returned listener immediately now stops it instead of detaching it.

The refresh single-flight contract is preserved in both SDKs. Cancelling a
waiter does not cancel a rotation whose result may need persistence. The new
checks distinguish that legitimate owner from cancelled consumers. Custom
storage/fetch adapters still own the responsibility to finish their operations;
this review does not impose a new timeout or discard accepted credentials.

## Validation

| Scope | Result |
|---|---|
| Catalog ownership tests | 3 passed: 32 failed reconciliations, 32 reconnect listeners, and cancellation while a worker waits indefinitely for a pool slot. Task counts return to baseline; discarded handles/state no longer upgrade through Weak references. |
| PostgreSQL integration | 5 passed across memory/architecture suites. Eight listener start/stop cycles release the catalog and leave zero matching PostgreSQL connections, even with introspection blocked by a held pool slot. Existing reconciliation and SQL cancellation/limits still pass. |
| Rust SDK auth | 8 passed, including 100 cancelled waiters retaining one rotation owner, and auto-refresh stopping when its handle is dropped. |
| TypeScript SDK unit checks | 96 passed across auth, HTTP and cancellation. Active results, independent cancellation, already-aborted callers and late rejection remain covered. TypeScript checking passes. |
| Built TypeScript SDK memory check | Three rounds of 1,000 cancelled callers: zero retained signals before the shared operation completes. The active waiter receives the accepted result, and completed operations become collectible. |
| Panel unit checks | 25 passed plus the separate GC-enabled query suite: 7 passed, including zero retained signals after 1,000 cancellations while another reader remains active. Overlapping query tests are not additional distinct cases. |
| Chromium | 15 passed across previews, upload replies and conflict flows. The two new memory cases each run three 1 MiB uploads and collect the original File after either cancellation or replacement. Existing destination capture, cancellation, response budgets and session fences pass. |
| Static/build checks | Rust formatting, targeted Clippy with warnings denied, capability boundaries, UI checking, translation checking, native base/import presets, SDK size budgets and embedded panel build pass. |

Memory checks are part of the UI CI job and the TypeScript SDK jobs for Node
22/24/26. Remote CI results are tracked by the follow-up pull request; the
validation table above records local evidence.
The UI GC test runs separately with `--execArgv=--expose-gc`; ordinary unit runs
skip that GC-dependent case, while the dedicated memory command must execute it.

The SDK observer mechanism adds 187–204 bytes gzip per entry point, with no new
runtime dependency. Each explicit size budget was raised by 200 bytes to account
for that measured correction; feature import separation remains checked.

| SDK entry | Before, bytes gzip | After, bytes gzip | Budget, bytes gzip |
|---|---:|---:|---:|
| Root | 16,789 | 16,976 | 17,100 |
| REST | 9,183 | 9,387 | 9,550 |
| Auth | 10,967 | 11,154 | 11,250 |
| Storage | 7,069 | 7,267 | 7,400 |

## Inspected bounds and limits

The reviewed paths already include a 50,000-key rate limiter cap, a 100-entry
denial log, 1,440 retained metrics minutes, 32 update sessions/plans, query cache
expiry after 60 seconds and clearing at session changes. Native session journal
transactions close their database connections. SQL/export/upload/webhook work
uses resource budgets and cancellation owners; process-wide health/router
listeners are installed once. These intentional caches and active work are not
classified as leaks merely because they retain memory.

The measurements above establish object/task/connection release in the tested
lifetimes. They do not establish zero leaks in every dependency or route, and
they are not a prolonged production RSS soak, a native allocation profiler or
Firefox/WebKit heap validation. Allocator retention and normal cache warming can
keep RSS above its starting value even after owners are released. The backend
continues to ship as a Rust binary with no Node runtime requirement.

Reproduce the focused checks using the environment/test limits in `AGENTS.md`:

```sh
scripts/test.sh rust -p nelcota-api catalog::
scripts/test.sh rust -p nelcota-server --test memory --test architecture
scripts/test.sh rust --manifest-path sdk/rust/Cargo.toml --test auth
cd sdk/typescript
npm run build && npm run test:memory
cd ../../crates/admin/ui
npm run test:memory
npx playwright test e2e/memory.spec.ts --workers=1
```

Run Rust tests and Vitest sequentially. Browser tests use Chromium; a local
system installation can be selected with `PLAYWRIGHT_CHROMIUM`.
