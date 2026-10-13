# Coordination log

## 2026-10-14 — Codex — Wafer package foundations

User requests a Cargo-like program named wafer, with packages named dies;
registry server later. Compiler resolution was implemented first in the isolated
siox-module-inputs worktree and its complete gate is still running. Own this
new separate local repository: strict Die.toml manifests, non-destructive
scaffolding, local dependency graph/version validation, Wafer.lock and JSON
metadata. Reuse Siox identifier validation through its LLVM-free compiler
dependency. No separate parser/compiler pipeline, package server, remote repo
creation or push. Git sources, native compiler execution and caching remain
required follow-up work toward the full goal, not waived deliverables.

Foundation gate729199 exited/success/0, peak794484736 bytes. Eight real CLI/graph
tests cover non-destructive new/init, JSON metadata, shared dependency ordering,
cycles/versions/name conflicts, source containment, stable locked-mode behavior,
manifest discovery and file-lock release. Strict all-target Clippy/fmt and
Graphify AST-only refresh pass (69 nodes/102 edges). LLVM stays disabled in the
compiler Git dependency. This does not verify Git fetching or native builds;
those commands are not implemented yet. Next own Git source/cache/lock support
and actual compiler orchestration. No remote repository or push was created.

## 2026-10-14 — Codex — Git dies and reproducible resolution

Own dependency manifest validation, project resolution/lock records, CLI
offline/update options and new Git cache module/tests. Pins belong in
Wafer.lock; normal commands reuse pins and update is explicit. Git path
dependencies must stay inside their locked checkout. Reject dirty caches
without deleting edits, unsafe transports and option-like revisions. Test
real local Git repos, including moved refs and unavailable remotes. No registry
server or compiler pipeline changes. Keep the compiler module-input commit
pending the user's baseline-gate decision; it is not a blocker to this work.

Git gate752660 exited/success/0, peak110501888 bytes:16 tests total (8 package
and8 actual-Git scenarios), strict all-target Clippy/fmt pass. Source pins keep
the exact commit until update; detached worktrees share objects with the mirror.
Offline/missing-remote sessions retain pins, failed/locked updates leave the
lock unchanged, dirty checkouts preserve edits, and Git sibling path dies
inherit the same revision. Helper transports, embedded credentials, bad source
specs/revisions/subdirectories, untracked source inputs and checkout escapes
are rejected. SHA-256 identifies URL cache directories; the sha2 dependency
is for stable cache identities, not a custom Git object hash implementation.
Graphify AST-only refresh completed (102 nodes/204 edges). Compiler execution,
build caching and full native acceptance remain required; this is not the whole
goal. No compiler/paper changes, remote repository or push in this batch.

## 2026-10-14 — Codex — real compiler check/build/test orchestration

Own manifest toolchain/test targets, package-owned source lists, runner module,
CLI and native integration tests. Run the external sioxc with explicit source
locations through its existing pipeline. Query its canonical test-plan artifact
and source-owned selection (added in the isolated compiler worktree), not raw
attribute scraping. Keep compiler errors/native failures visible; preserve
source paths and select dependency tests out of package runs. Native object
builds use the compiler's root/top selection; source-only libraries use check.
Implement filtering/listing and per-target VCD/FST paths. Build cache remains a
separate next slice. No fake compiler fixtures for the native acceptance gate.

Native806207 exited/success/0, peak158724096 bytes:16 regular tests and5 actual
compiler/native tests pass, strict all-target Clippy/fmt. Fresh new/init source
now has Top + Smoke and builds/tests without manual edits. Local and Git dies
compile/run through the actual source-input compiler, including unavailable
remotes with offline locked pins. Object output, no-test libraries, ownership
selection, exact once-per-target execution, listing/filtering, VCD/FST files and
both source/native failures are verified. Compiler781291 also passed508 frontend
library tests, module-input cases,2 frontend and3 native test-plan checks plus
strict Clippy before Wafer integration. Canonical discovery remains in sioxc;
no LLVM/runtime or second language pipeline was added. Graphify refresh passes
(140 nodes/296 edges). Compatible compiler features are still uncommitted and
unpublished pending the owner/baseline decision; README states this explicitly.
Build-cache/safe-clean, project configuration and full release CI remain.

## 2026-10-14 — Codex — bounded artifact caching and safe cleanup

Own cache/fingerprint helpers, declared build inputs, runner reuse and clean
CLI/tests. Cache is opt-in while the compiler lacks complete dependency reporting;
outside compile-time inputs must be explicitly declared. Hash content, not just
timestamps, across die roots, std, compiler/tool executables, command and process
environment; record only hashes, not environment secrets. Missing/changed output
or corrupt stamps rebuild, failures never create a hit, and test results/waves
are never cached. Clean removes only target/wafer, rejects ancestor symlinks,
and retains source, lockfiles, Git checkouts and unrelated target contents.

Cache/clean882362 exited/success/0, peak161927168 bytes:19 regular tests and8
actual compiler/native scenarios, strict all-target Clippy/fmt pass. Content
changes in dependency/std/compiler/extra inputs invalidate fingerprints;
missing/corrupt outputs and malformed stamps rebuild. Test binaries may be
reused but still execute. Fresh object/binary hits are observed with actual
sioxc, and --no-cache bypasses them. Before/after hashes reject changing input
snapshots. Symlink tests preserve outside target/object/wave sentinels. Clean
works with missing dependencies and invalid/unread lock content, preserving
source/Git caches/unrelated artifacts. Linker hashing applies only to native
test linkage; object-only fingerprint units require no Clang install. Runtime
objects are embedded in sioxc, confirmed in src/driver/build.rs, so compiler
identity covers those bytes. Graphify refresh passes (168 nodes/361 edges).
Cache is off by default and explicitly documents declared-input/toolchain limits;
the user choice about compiler dependency reporting remains unanswered. No
compiler/paper edits, remote repository, server or push in this batch.

## 2026-10-14 — Codex — project configuration and release checks

Own project-config JSON export, CI workflow and reproducible acceptance script.
Export resolved compiler/std/source settings through the same runner config;
do not implement package discovery inside LSP or claim its adapter consumes
new settings already. Native CI requires an explicitly supplied compatible
compiler revision while prerequisites remain unpublished. Regular CI remains
LLVM-free; the local native acceptance gate uses the actual development binary.
Keep full goal active until prerequisites/acceptance are actually resolved.

Project/CI890153 exited/success/0, peak154013696:19 regular tests and9 actual
compiler/native cases pass through scripts/ci-local.sh with native required.
Project-config JSON matches runner compiler/std/entry/module settings and
excludes dependency sources from package test roots. Bash syntax/YAML parsing
checks pass; GitHub Actions was not run and no remote repo exists. Regular CI
is LLVM-free; manual native CI takes an explicit compatible compiler_ref and
builds the real compiler with its libraries. No invented published commit or
silent native skip. Graphify refresh passes (176 nodes/374 edges).
Goal remains incomplete pending compiler integration/owner-gate decision and
the final compatible-revision acceptance audit. No push or server work.

## 2026-10-14 — Codex — compiler acceptance handoff and final script verification

Compiler full896583 passes complete default/bitpack Rust suites,23 DWARF cases
in each mode and strict Clippy/fmt/build, but both246-file corpora remain241/5.
All five failures were independently reproduced on unchanged3274566. Compiler
publication still awaits the owner decision on the baseline commit-gate
exception; do not claim the active package-tool goal completed. Own this log
entry and final script verification against the just-tested actual compiler.
Verify the portable grep checks and required-native rejection after da18044;
no compiler changes, registry/MCP server, remote creation or push.

Final967499 exited/success/0, peak97697792 bytes:19 regular tests and all9
actual compiler/native scenarios pass against the final bitpack compiler
artifact, with strict Clippy/fmt. The required-native missing-configuration
branch deliberately returned2 as expected; the surrounding verification
checked that exact status and succeeded. Portable grep checks work. No fake
compiler, skipped native acceptance or claim of published/remote CI coverage.
Wafer implementation remains approximately90%, blocked on compiler prerequisite
commit/publication and configured native CI execution, not its own local gate.

