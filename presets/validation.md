# Review preset baseline

Oxlint 1.87.0, Linux x64, two native workers, sequential execution. Recorded on 2026-10-12 against application commit c9c70e2. Config/scripts only; no application source or embedded assets were changed and no actual Oxlint --fix was executed.

18 applicable presets completed (17 native Oxlint presets and the npm audit adapter); there were no tool execution failures. Counts are diagnostics per preset, including repeated findings across themes.

| Group | Preset | Findings | Time (ms) | Execution |
| --- | --- | ---: | ---: | --- |
| bugs | correctness | 12 | 316.810 | Completed |
| bugs | nan | 13 | 30.216 | Completed |
| bugs | nulls | 0 | 46.812 | Completed |
| bugs | vazios | 24 | 36.733 | Completed |
| bugs | promises | 12 | 37.493 | Completed |
| bugs | nulls-tipados | 0 | 0.000 | Disabled: tsgolint missing |
| bugs | promises-tipadas | 0 | 0.000 | Disabled: tsgolint missing |
| seguranca | execucao-dinamica | 0 | 29.980 | Completed |
| seguranca | regex | 1 | 40.635 | Completed |
| seguranca | debug-esquecido | 31 | 36.066 | Completed |
| performance | async-em-loop | 49 | 40.154 | Completed |
| performance | imports | 4 | 340.635 | Completed |
| performance | eficiencia | 5 | 32.031 | Completed |
| limpeza | codigo-morto | 1 | 31.367 | Completed |
| limpeza | consistencia | 764 | 44.257 | Completed |
| limpeza | typescript-limpo | 209 | 40.801 | Completed |
| limpeza | pendencias | 0 | 40.211 | Completed |
| complexidade | early-return | 2 | 29.638 | Completed |
| complexidade | tamanho | 71 | 38.140 | Completed |
| seguranca | segredos | 0 | 0.000 | Unavailable: tool missing |
| seguranca | dependencias | 0 | 897.199 | Completed |
| seguranca | padroes-inseguros | 0 | 0.000 | Unavailable: tool missing |
| seguranca | xss-react | 0 | 0.000 | Not applicable: no React/JSX |
| performance | react-perf | 0 | 0.000 | Not applicable: no React/JSX |
| acessibilidade | a11y | 0 | 0.000 | Not applicable: no React/JSX |

Total: 1167 error-level diagnostics and 31 warnings. The all command exits 1 because error-level diagnostics exist; this is distinct from an engine execution failure.

## Noisy presets: proposed tuning, not applied

- limpeza/consistencia: 764 findings (763 curly and 1 prefer-const). Review whether the project wants braces around every one-line guard. Possible choices: curly multi-line or warning severity for style rules. No rule options or severities were changed to reduce the baseline.
- limpeza/typescript-limpo: 209 findings. Review non-null assertions in tests separately from production code; a test-only warning override is one possible policy. The initial requested rules remain error-level.
- complexidade/tamanho: 71 findings at the requested complexity 10 / function length 60 / parameter count 4. Consider a separate test-file policy before raising global limits. The requested limits are unchanged.

## Findings that need context

- unicorn/no-thenable flags the intentionally awaitable SDK query builder. Confirm its public API contract before changing it.
- eslint/no-control-regex flags the download-name sanitization expression, which intentionally removes control characters. Review that boundary before adding a narrow suppression.
- NaN findings include global NaN literals in regression tests. Existing Number.isNaN boundary checks are preserved.
- Import and async-loop findings need a review of ownership and required serialization; automated concurrency changes were not performed.

Generated files, dist, node_modules and build output contributed zero findings after root-level exclusions were verified.

## Unavailable and inactive coverage

- Gitleaks and Semgrep were not installed; their adapters return unavailable. Project-local install commands are in README.md.
- Type-aware nullable conditions and floating promises remain disabled until a compatible native tsgolint is supplied.
- React XSS, React performance and JSX accessibility are not applicable to this Svelte project. Svelte template/a11y checking remains with svelte-check.
- npm audit completed for both existing lockfiles with zero findings. It remains an optional Node-dependent adapter; native Oxlint/Python execution was tested with an empty PATH.

## Tooling verification

- 13 Python tests passed, including native preset parsing, ignore paths, JSON output, severity/exit handling, redaction and concurrent-edit protection for mocked fixes.
- Every configured rule name exists in the pinned native registry.
- Native installation SHA-256 verified; all rule config JSON files accepted by Oxlint.
- The application distribution and tracked original source files are unchanged.
