# Cargo-based Wafer adaptation

- [x] Bootstrap the isolated fork and a Wafer executable; preserve upstream Cargo.
- [x] Discover Wafer.toml, workspace members and inherited metadata using Cargo code.
- [x] Infer Siox library/test/binary/example/benchmark paths using Cargo's discovery.
- [x] Isolate Wafer home/project config and typed configuration environment keys.
- [x] Resolve explicit path/Git dependencies into Wafer.lock using Cargo's resolver/cache/encoder;
  honor offline/locked/frozen and reject registry access before connections.
- Adapt external commands and remaining compiler/registry environment contracts.
- Adapt source-file enumeration/fingerprints and conservative lock reuse for future builds.
- Wire Sioxc version/native-target/artifact-name queries into Cargo planning;
  extend capabilities as the compiler provides compiled-library/dep-info contracts.
- Adapt compilation, dep-info, diagnostics and artifact types; no rustc spoofing.
- Compile/reuse Siox dependencies with version-isolated identities and metadata.
- Adapt native test discovery/batching and simulation/structural top selection.
- Carry project-config/LSP boundary and output safety from the existing Wafer.
- Verify inherited resolution/features/Git/lock/update/profile/scheduling behavior.
- Define Siox registry boundaries; servers and publishing remain later work.
- Audit actual redistributed dependencies/notices and validate Windows/macOS.

This is a new integration effort, not the earlier standalone Wafer's60% estimate.
Copying upstream functionality does not prove it works with Siox.

Verified adapter: 17 executable CLI/Git tests, one actual Sioxc probe integration,
147 library tests (143 upstream, native selection explicitly enabled), formatting
and strict relevant-target Clippy. Cargo-driven compilation, compiled dependency
libraries and full metadata remain unsupported, not silently delegated to Rust. Compiler query
changes remain local until its separate full corpus gate is green.
