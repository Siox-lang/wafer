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
Build reuse/caching is not implemented yet.

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

All 16 regular tests and five real compiler/native tests pass, along with strict
Clippy/fmt. Native acceptance covers a fresh scaffold, local/Git dependencies,
offline locked compilation, objects, canonical test listing/filtering/ownership,
VCD/FST output, empty libraries and compiler/runtime failure propagation.

