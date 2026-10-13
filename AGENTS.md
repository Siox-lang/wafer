# Wafer repository instructions

Read README.md, TODO.md and the latest chat.md entries before changing code.
Check git status and preserve other agents' changes. Log shared-file ownership
in chat.md. Use the compiler's API or CLI, never a second language pipeline.
Wafer owns die manifests, dependency/version resolution and project execution;
sioxc remains manifest-independent and registry/MCP servers are separate work.

On the shared development host, use ../siox/scripts/guarded-run.sh for builds
and integration tests, one job at a time. Inspect its retained terminal result.
Before committing, run tests, strict all-target Clippy and fmt, and exercise
real sioxc builds when compiler orchestration changes. Include a scoped
Completion estimate in every commit. Push only when explicitly requested.

Use the installed Graphify skill for codebase navigation when a graph exists,
and run graphify update . after code changes. Generated graphs are ignored.

