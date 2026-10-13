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

