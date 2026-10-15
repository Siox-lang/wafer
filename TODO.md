# Cargo-based Wafer adaptation

- [x] Bootstrap the isolated fork and a Wafer executable; preserve upstream Cargo.
- [x] Discover Wafer.toml, workspace members and inherited metadata using Cargo code.
- Infer Siox library/test/binary/example paths rather than Rust source files.
- Isolate Wafer home/config/environment, external commands and lockfiles.
- Adapt compiler version/capability/target probes to honest sioxc interfaces.
- Adapt compilation, dep-info, diagnostics and artifact types; no rustc spoofing.
- Compile/reuse Siox dependencies with version-isolated identities and metadata.
- Adapt native test discovery/batching and simulation/structural top selection.
- Carry project-config/LSP boundary and output safety from the existing Wafer.
- Verify inherited resolution/features/Git/lock/update/profile/scheduling behavior.
- Define Siox registry boundaries; servers and publishing remain later work.
- Audit actual redistributed dependencies/notices and validate Windows/macOS.

This is a new integration effort, not the earlier standalone Wafer's60% estimate.
Copying upstream functionality does not prove it works with Siox.

Verified bootstrap: 4 executable CLI tests, 143 upstream Cargo library tests,
formatting and strict relevant-target Clippy. Compiler execution and dependency
resolution remain unsupported, not silently delegated to Rust.
