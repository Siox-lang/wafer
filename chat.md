# Wafer Cargo-fork coordination

## 2026-10-14 — Codex: authorized isolated fork and project discovery

Owner chose an isolated Cargo fork that we edit. Forked the pinned upstream
Rust1.90 branch commit840b83a10fb0e039a83f4d70ad032892c287570a into
/home/max/wafer-cargo, branch feat/siox-adapter, remote upstream pointing to
rust-lang/cargo. The source snapshot is shallow; upstream commit ancestry
can be fetched later without replacing this baseline. No GitHub fork/push yet.
../wafer and its incomplete submodule work are preserved; the pristine cache
checkout is unchanged. Preserve upstream license and third-party notices.

Own initial Wafer binary/context, shared manifest/workspace discovery and
metadata CLI tests/docs. Reuse Cargo implementations; keep the Cargo executable
as a reference. Initially enable metadata --no-deps and locate-project, with
explicit Siox target paths, and reject compiler/registry/mutation operations
before dispatch until their boundaries are adapted. Sioxc compilation is not
implemented by this bootstrap. No compiler/LLVM/runtime changes. Guarded build,
Wafer tests/relevant upstream regressions/fmt/Clippy before local commit.

Metadata bootstrap also omits Rust-only edition/rust-version/crate-type fields
at the shared CLI output boundary rather than advertising fictitious Siox
compiler properties. Upstream Cargo output stays unchanged. First guarded gate
is queued in exec session19705 behind confirmed live holders3133286/3133535;
inspect its returned systemd unit when the launcher completes. No test pass
or compiler support is claimed before that gate. Config/home environment
isolation and automatic Siox target discovery remain separate follow-up work.

## 2026-10-15 — Codex: bootstrap build fixes and verification

Own the bootstrap CLI/metadata compile fixes and gate follow-through. Queued
launch1792013972753847363-3152338 ended75 before compilation due to contention;
after both foreign holders exited, gate1792014100290824497-3163911 compiled
the dependencies and ended101, peak1610874880 bytes. The metadata adapter
needs serde_json::Error converted through anyhow::Error for Cargo's CliError.
Fix the shared adapter once and rerun the full fork gate; no test pass claimed
from that failed build. Original Wafer and compiler/LLVM work remain untouched.

Fixed serialization error propagation via anyhow::Error, preserving Cargo's
CliError handling rather than suppressing the error. Follow-up gate
siox-job-3000-1792071280173625711-3470283.service is launched; inspect its
retained terminal state/log. The previous foreign holder3466437 exited before
launch; no job was stopped or lock bypassed. Gate includes fork CLI tests,
upstream Cargo library tests, strict relevant-target Clippy/fmt, then a scoped
code-only graph refresh for src/ (no semantic API or visualization). Source
is frozen while the gate runs; no pass/commit claimed yet.

Follow-up1792071280173625711-3470283 ended75 on contention, without testing.
After checking that the lock holder exited, the next gate
siox-job-3000-1792071348092152681-3473924.service finished exited/success/0,
peak1447702528 bytes: 4 actual executable CLI tests, 143 upstream library tests,
formatting and strict relevant-target Clippy all pass. Scoped graph refresh
completed (8670 nodes, 19627 edges at update time). No compiler execution,
native simulation, full upstream integration or platform acceptance claimed.
Checkpoint only: Cargo-based Siox integration is approximately5% implemented;
compiler, dependency, test, configuration and artifact adapters remain open.

## 2026-10-15 — Codex: Siox target auto-discovery

Own shared targets.rs inference, its manifest-normalization callers and Wafer
CLI regression tests/docs. Thread the active source extension through Cargo's
existing discovery/diagnostic code; keep Cargo .rs behavior and legacy paths
unchanged. Wafer should infer .siox targets with Cargo's layout/overrides, not
Rust files, Rust2015 legacy fallbacks or automatic build.rs scripts. Explicit
Wafer build scripts remain rejected until a build-script adapter exists. No
compiler execution or dependency resolution is being enabled by this step.

Verification1792071751871216366-3497014 ended75 on the shared lock without
running checks. Retry queued in exec session50006 behind confirmed live holder
3496774; poll that session for the next unit/log, without restarting live jobs.
Source is frozen for the queued fmt/tests/Clippy/graph gate. No pass claimed.

Gate1792071789514906921-3497877 ended101, peak1304297472 bytes: compilation
and7/8 CLI tests passed, but a new diagnostic test incorrectly expected metadata
to fail for invalid optional test targets. Cargo intentionally defers those
errors while returning usable targets. Preserve that behavior; assert CLI
partial metadata and test deferred .siox diagnostic text directly in the shared
normalizer. No production error handling is removed to satisfy this correction.

Corrected gate siox-job-3000-1792071898357388500-3505206.service finished
exited/success/0, peak1052573696 bytes: 8 executable CLI tests, 144 library tests
(143 upstream plus one language-specific normalizer regression), fmt and strict
relevant-target Clippy pass. Graph refresh completed (8671 nodes/19630 edges).
Tests compare actual Cargo/Wafer discovery, explicit/default targets, auto flags,
Rust-file exclusion, deferred invalid test targets and unsupported build scripts.
Checkpoint estimate approximately8% of Cargo-based Siox integration: discovery
is verified, but compiler, dependencies, configuration, artifacts and native
test execution remain unadapted. No full upstream integration/platform/native
simulation acceptance or push claimed. Next boundary: isolated Wafer config
and honest Sioxc capability/target probes, then shared compilation planning.

## 2026-10-15 — Codex: Wafer configuration isolation

Own GlobalContext home/config discovery, typed environment-key constructors,
their config/target/auth callers, active binary environment entry points and
CLI regression tests/docs. Reuse Cargo config parsing/merging with .wafer,
WAFER_HOME and WAFER_* keys; do not rewrite the process environment or expose
Cargo user config/credentials to Wafer. Keep the reference Cargo behavior.
Also close the inherited rustfix proxy entry point for Wafer so an environment
variable cannot bypass the unsupported-compiler guard. Dependency resolution,
external command dispatch and native compilation remain gated/unadapted.

Gate siox-job-3000-1792072282177982284-3525714.service ended101, peak1362223104
bytes. All11 actual CLI regressions passed; the new library test had an ambiguous
unnecessary .into() at generic set_search_stop_path. Pass &Path directly and
rerun the same gate. No whole-gate pass or commit claimed from this failed run.

Retry1792072346372999316-3529719 ended75 on contention. Queued exec56781
behind confirmed live holder3529211, then inspected its returned gate
siox-job-3000-1792072380415871959-3530649.service: exited/success/0,
peak1124179968 bytes. All11 executable CLI tests,145 library tests (143 upstream),
formatting and strict relevant-target Clippy pass. Graph refresh completed
(8675 nodes/19648 edges). Tests cover Cargo/Wafer home and config isolation,
relative/absolute/empty WAFER_HOME, typed/direct environment keys, CLI config
precedence, aliases/unsupported operations, key push/pop and error origins,
home changes forbidden through [env], and the inherited rustfix-proxy bypass.
No native Sioxc, full upstream integration suite or platform acceptance claimed.
Scoped engineering estimate approximately10% of Cargo-based Siox integration;
compiler/version/target probes, dependencies, lockfiles, artifacts and native
tests remain open. The active goal remains unfinished; no push was requested.

## 2026-10-15 — Codex: compiler-owned toolchain query boundary

Current cached Sioxc main lacks target/artifact queries; Cargo's Executor cannot
replace those earlier Rust-specific planning probes. Own an isolated compiler
worktree /home/max/siox-toolchain-queries from cached origin/main88e3d4c,
not Claude's LLVM/IR work or the existing dirty compiled-units driver. Add real
compiler-owned queries and consume them from the fork, with native/query tests;
do not fabricate Rust capabilities or add a separate Wafer planner. Keep compiler
changes uncommitted until the full Sioxc house gate is green. Fork/compiler
builds remain guarded and serialized. No push or build support claimed yet.

Sioxc query implementation is local in feat/toolchain-queries, with shared
FileArtifact default names and actual build-host/LLVM metadata. Its full gate
siox-job-3000-1792073275469765838-3577911.service is confirmed running:841 Rust
tests passed, default corpus now executing; bitpack and final checks remain.
Own fork util/sioxc.rs, public context loader and actual native probe tests.
The loader selects SIOXC/build.sioxc/PATH and does not fake Rust identity or
formats. Native acceptance tests are explicitly ignored unless selected with
a real WAFER_TEST_SIOXC; no silent-pass fallback. Queued fork gate exec21616
waits on confirmed live PID3577923, then runs11 CLI tests, actual native probe
integration and library tests with --include-ignored, strict Clippy/fmt and
graph refresh. Poll that same session for its unit/log. Both source trees are
frozen; no adapter acceptance, full corpus pass, compiler/fork commit or push
claimed yet. Cargo unit planning remains unadapted and build commands gated.

Compiler full3577911 is terminal exit1, no OOM (peak1537024000 bytes):841 Rust
tests and the additional frontend-only metadata tests pass, as do all-feature
Clippy/fmt and scoped graph refresh, but both actual corpora are241/246 with the
same five failures. Compiler changes remain uncommitted. Exact unchanged88e3d4c
baseline comparison is running as siox-job-3000-1792074224666401596-3654471.service;
do not claim pre-existing failures until it reproduces the matching messages.
Cached origin/main was inspected/fetched and is nowa01f024 (#108), but the
query worktree/base were not modified during verification.

Queued exec21616 launched siox-job-3000-1792074156363090831-3585285.service,
now exited/success/0 (peak1407860736 bytes):11 actual CLI tests,1 actual Sioxc
probe integration and147 library tests including both previously ignored
acceptance checks explicitly enabled. Fmt/strict relevant-target Clippy and
graph refresh pass (8688 nodes/19696 edges). Probe/selection tests use the real
local compiler and reject the actual Cargo executable as Sioxc. No fake
compiler, native project build, reusable library, full Cargo integration suite,
compiler house-gate pass or push is claimed. Approximate Cargo-based integration
estimate12%; planning, build/dependency/cache/artifact/test adapters remain.

Exact baseline3654471 is now exited/success/0, peak926982144 bytes: all five
failures reproduce with the same messages under default and bitpack (10/10)
on clean88e3d4c. This proves their presence before the compiler query patch,
not a passing full compiler gate. Keep compiler changes local/uncommitted.
The separately green fork probe boundary is ready for a scoped local commit;
its actual compiler dependency is clearly documented as an unreleased local
interface, and all build/check/test planning remains gated. No push requested.
