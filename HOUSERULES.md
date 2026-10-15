# Wafer Cargo-fork house rules

Retain Cargo's resolver, source management, configuration, unit planning,
scheduling and fingerprinting. Sioxc alone owns Siox parsing, type checking,
elaboration and code generation; do not create another language pipeline.
Library names are single namespaces, not vendor::lib. No modules manifest table.
Registry/MCP servers remain separate and later; do not contact crates.io for
Siox dependencies before a Siox registry contract exists.

Keep upstream Cargo behavior available as the reference executable. Enable
Wafer adaptations explicitly; never spoof rustc output or send Siox to rustc.
Keep the fork's Rust build manifest separate from the Siox project manifests.
Preserve LICENSE-MIT, LICENSE-APACHE and applicable third-party notices.
Use Rust1.90 for this pinned baseline, retaining Cargo's upstream edition2024.
Do not add dependencies when upstream already provides the required facility.

Before commits, run formatting, relevant upstream regressions and Wafer-specific
integration tests, plus strict Clippy for modified targets. Compiler execution
changes also need actual sioxc/native acceptance. Unsupported operations must
fail clearly until implemented. A passing metadata check is not build support.
Launch heavy work through ../siox/scripts/guarded-run.sh and inspect terminal
SubState=exited, Result=success, ExecMainStatus=0. Do not bypass a busy lock or
stop another agent's verification. Preserve uncommitted work, append ownership
to chat.md, include Completion estimate in commits, and request pushes separately.
