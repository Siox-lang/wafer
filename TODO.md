# Cargo-based Wafer adaptation

- [x] Bootstrap the isolated fork and a Wafer executable; preserve upstream Cargo.
- [x] Discover Wafer.toml, workspace members and inherited metadata using Cargo code.
- [x] Infer Siox library/test/binary/example/benchmark paths using Cargo's discovery.
- [x] Isolate Wafer home/project config and typed configuration environment keys.
- [x] Resolve explicit path/Git dependencies into Wafer.lock using Cargo's resolver/cache/encoder;
  honor offline/locked/frozen and reject registry access before connections.
- [x] Produce full metadata through Cargo's existing graph/feature resolver using
  actual Sioxc host facts; conservatively reuse path/Git lock resolutions.
- [x] Select source/freshness inputs through Cargo's existing filesystem/Git walker
  with active manifest/lock names, nested boundaries and include/exclude rules.
- [x] Share root/dependency unit planning and host-unit deduplication with Sioxc;
  verify versioned libraries, features, native cfg, filters and profiles via API.
- [x] Use one shared build-context planner for both compilers and expose explicit
  `build --unit-graph` CLI planning; preserve Rust std/MSRV and reject unsupported options.
- [x] Decode actual compiler-owned Sioxc dependency sidecars into Cargo's shared
  encoded input format; verify rebasing, aliases and corruption handling via native API tests.
- [x] Select configured Sioxc flags through Cargo's shared precedence and unit identity;
  verify encoded/plain environments, config arrays, native cfg and host isolation.
- [x] Share compiler/native process environment setup through Cargo's `Compilation`;
  verify actual Sioxc object/test/VCD execution, package variables and Rust-only guards.
- [x] Share native artifact names/unit namespaces/output-name caching and planned
  uplift/export paths through `CompilationFiles`; verify real object/test outputs.
- [x] Share runner state, jobserver/layout preparation and dry-run output collection;
  distinguish native objects from executables and reject unsupported artifacts/LTO early.
- [x] Include actual compiler executable contents in unit namespaces and fingerprints;
  verify identical versions/sizes/mtimes cannot conceal changed executable bytes.
- [x] Share persisted fingerprint comparison and fresh/dirty job preparation for native
  units; verify real inputs, missing/corrupt sidecars and interrupted-job invalidation.
- [x] Share the actual queue/jobserver/drain paths for native work; verify one/two-job
  limits, ordering, fresh skips, rebuilds, failures/keep-going and both compilers' timings.
- Adapt external commands and remaining compiler/registry environment contracts.
- Integrate dependency-sidecar collection and build-unit fingerprints with Siox execution.
  Track execution environment/selected std and external tools/libraries, not only
  the already captured compiler binary, before enabling native build-cache reuse.
- Integrate native work preparation into the high-level shared runner; wire actual compiler artifact
  names and selected Sioxc flags without Rust crate formats. Validate build-owned
  output/query options before constructing an executing command.
- Adapt compilation, dep-info, diagnostics and artifact types; no rustc spoofing.
- Compile/reuse Siox dependencies with version-isolated identities and metadata.
- Adapt native test discovery/batching and simulation/structural top selection.
- Carry project-config/LSP boundary and output safety from the existing Wafer.
- Verify inherited resolution/features/Git/lock/update/profile/scheduling behavior.
- Define Siox registry boundaries; servers and publishing remain later work.
- Audit actual redistributed dependencies/notices and validate Windows/macOS.

This is a new integration effort, not the earlier standalone Wafer's60% estimate.
Copying upstream functionality does not prove it works with Siox.

Verified adapter: 23 executable CLI/Git/metadata/planning tests, four source-tracking
regressions for both tools, two actual Sioxc toolchain integrations,
171 library tests (143 upstream, native selection/planning/dep-info/process/artifacts/runner/fingerprints/scheduling explicitly enabled), formatting
and strict relevant-target Clippy, including a real Cargo reference native build.
Cargo-driven Siox compilation and compiled dependency libraries remain
unsupported, not silently delegated to Rust. Compiler query
changes remain local until its separate full corpus gate is green.
