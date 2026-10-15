# Wafer Cargo-fork instructions

Read HOUSERULES.md, README.md, TODO.md and the latest chat.md before editing.
Check git status; preserve other agents' changes and log ownership first.
Use the existing Cargo implementations, not parallel resolver/config/planner code.
Preserve upstream licensing and mark modified upstream files with a short notice.
This fork is separate from ../wafer and the pristine upstream cache checkout.
Do not push unless explicitly requested. Every commit includes a scoped
Completion estimate with evidence and remaining gaps.
Use ../siox/scripts/guarded-run.sh for heavy builds/tests, one job at a time;
inspect its terminal systemd result, not just launch status. Read HOUSERULES.md
for the gate. The code-only graph is scoped to src/, at
src/graphify-out/graph.json; use graphify query --graph with that path and
graphify update src --no-cluster after changes. Generated graphs are ignored.
