# Wafer

Wafer is the Cargo-like project tool for Siox. Packages are called **dies**.
The compiler remains `sioxc`; package discovery, versions, caching and execution
belong here. The die registry server and MCP adapter come later in separate repos.

This checkout is under development, not a completed Cargo replacement.
The initial package boundary is `Die.toml` and `Wafer.lock`:

```toml
[die]
name = "spi_controller"
version = "0.1.0"

[lib]
path = "src/lib.siox"

[modules]
"spi_controller::helpers" = "src/helpers.siox"

[dependencies]
bus = { path = "../bus", version = "^0.1" }
# Git alternatives (subdir/rev are optional):
# timing = { git = "https://example.com/timing.git", rev = "v0.1", version = "^0.1" }
```

The root source declares `module spi_controller;`. Module keys are complete
Siox namespace paths; the compiler checks their declarations. Package names
may contain hyphens, which become underscores in the default root namespace.
Paths in `[lib]`/`[modules]` stay within their die; dependency paths may point
to sibling dies. Packages and compile-time source operations must be trusted.

Initial commands: `wafer new <directory>`, `wafer init [directory]`,
`wafer metadata [--manifest-path <Die.toml>] [--locked]`, `wafer fetch` and
`wafer update`. `--offline` forbids Git network access. Normal commands keep
the commits recorded in `Wafer.lock`; only `update` moves existing pins.
Metadata is JSON on stdout for tools such as the LSP; status/errors go to stderr.
See TODO.md for unfinished build-cache and release-acceptance work.

Git sources accept HTTPS, SSH, `file://` or absolute local repository paths.
Use credential helpers/SSH agents rather than embed passwords in URLs.
Optional `subdir` selects a die within a repository. Path dependencies of a
Git die stay in that same locked checkout; manifests and module sources must
be tracked. Git mirrors and detached commit checkouts live in the root project's
ignored `.wafer/git/`, sharing Git objects. Dirty checkouts are rejected without
deleting edits. `update` cannot be combined with `--locked` or `--offline`.
Package aliases and multiple versions sharing a module namespace are rejected
until the compiler provides version-isolated identities; no implicit version
or source wins a conflict.

Rust 1.90 or newer is required. The compiler Git dependency disables LLVM and
is used for identifier validation only. Builds invoke `sioxc` with
explicit module inputs rather than copy packages or read manifests in sioxc.

## Check, build and test

The external compiler must support explicit `--module` inputs, `--emit test-plan`
and `--test-source` selection. These interfaces are currently implemented in the
compiler's `feat/explicit-module-inputs` development worktree; they are not yet
available in the published revision pinned for Wafer's identifier-only dependency.
Use a matching development binary and its standard library:

```bash
wafer --sioxc /path/to/sioxc --std /path/to/sioxc/std check
wafer --sioxc /path/to/sioxc --std /path/to/sioxc/std build
wafer --sioxc /path/to/sioxc --std /path/to/sioxc/std test
wafer --sioxc /path/to/sioxc --std /path/to/sioxc/std test --list
wafer --sioxc /path/to/sioxc --std /path/to/sioxc/std test 'my_die::Smoke' --wave both
```

Alternatively set `SIOXC`/`SIOX_STD`, or root-manifest `[toolchain]` fields
`sioxc` and `std` (manifest paths are relative to the die). CLI/environment
settings take priority. Wafer validates package sources but never implements
language semantics or executes code inside `sioxc` itself.

`check` analyzes every declared library module and the root die's test files.
`build` emits a native design object under `target/wafer/release/`; select its
entity with `--top` or `[lib].top`. It uses the compiler's normal structural-root
selection. Source-only function/type libraries are checked and imported as
source, not advertised as separately compiled native libraries.

`test` discovers `.siox` files recursively under `tests/`, plus explicit
`[[test]]` paths and library-owned tests (`[lib].test = false` opts out).
It queries canonical tests through the compiler, compiles only the selected
source-owned roots, and runs the resulting native binaries. Dependency-owned
tests do not run. An optional substring filter uses qualified test names;
`--list` prints them without code generation. Empty libraries succeed with no
native tests. Compiler and runtime errors propagate as a failing exit status.
`--wave vcd|fst|both` writes separate files per target under the test artifact
directory. `--debug` enables source DWARF and separate debug artifacts.
Fresh `new/init` dies include a buildable `Top` and canonical smoke test.
## Artifact cache and cleanup

`build --cache` and `test --cache` opt into artifact reuse. Successful objects
and test binaries have content fingerprints covering die roots, declared extra
inputs, std, the compiler (including its embedded runtime objects), the native
linker where needed, command options and the process environment. Fingerprints
are checked again after compilation. Missing/corrupt artifacts or stamps
rebuild; failed compilation never creates a cache hit. Test results and waveform
files are never cached: reused test binaries still execute. `--no-cache` bypasses
reuse. Diagnostics from successful compilations are retained on cache hits.

Caching is deliberately off by default while Siox lacks a complete compile-time
dependency report. Declare outside files/directories read during compilation:

```toml
[build]
inputs = ["../shared/rom.bin", "../shared/constraints"]
```

Only declared inputs are covered. Undeclared external files, mutable system
libraries or other outside toolchain inputs can invalidate that assumption;
declare them or keep caching disabled. Root trees exclude `.git`, `.wafer` and
`target` outputs; explicitly declared module sources are always fingerprinted.
Caching refuses directory-tree symlinks rather than guess their dependency set.
Environment values are hashed, not written into cache records.

`wafer clean` removes only `target/wafer`. It works without resolving/fetching
dependencies and leaves sources, `Wafer.lock`, `.wafer/git` and unrelated target
files untouched. Artifact directories, output files and waveform paths reject
symlinks so builds/cleanup cannot overwrite or delete a linked outside target.

## Verification

```bash
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
# Opt-in actual compiler/native acceptance, no fake compiler:
export WAFER_TEST_SIOXC=/path/to/compatible/sioxc
export WAFER_TEST_STD=/path/to/its/std
cargo test --locked --test compiler_execution -- --ignored --nocapture --test-threads=1
```

All 19 regular tests and eight real compiler/native tests pass, along with strict
Clippy/fmt. Native acceptance covers a fresh scaffold, local/Git dependencies,
offline locked compilation, objects, canonical test listing/filtering/ownership,
VCD/FST output, empty libraries, compiler/runtime failure propagation, content
invalidation, corrupt/missing cache entries, rerunning cached binaries and safe
cleanup/output paths.

