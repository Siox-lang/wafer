# Wafer — isolated Cargo fork

This checkout adapts Cargo for Siox, based on upstream commit
`840b83a10fb0e039a83f4d70ad032892c287570a` (Rust1.90 branch). It is separate
from the existing standalone Wafer implementation and is not a finished release.
The Rust build package/library remain named cargo to keep upstream integration
and its locked dependency graph intact. The new executable is named wafer.

Siox projects use Wafer.toml and library names directly as namespaces. Bootstrap
commands are `wafer locate-project`, `wafer metadata --format-version 1`
and `wafer generate-lockfile`. Explicit planning is available with
`wafer build --unit-graph -Zunstable-options`, retaining Cargo's unstable-channel
gate; it does not compile source or produce build artifacts.
Source targets follow Cargo's layout with `.siox` instead of `.rs`: `src/lib.siox`,
`src/main.siox`, and files or `<name>/main.siox` below `src/bin/`, `examples/`,
`tests/` and `benches/`. Explicit target paths and `auto* = false` overrides
are supported. `[lib] name = "vendor"` sets the single library namespace.
Rust legacy paths and automatic `build.rs` discovery do not apply to Wafer;
explicit build scripts are rejected until their adapter exists. Compilation
and remaining mutation/registry commands still need adaptation. They are not implied
by the inherited Cargo help/API or resolver code. See TODO.md and chat.md.

Bootstrap metadata uses Cargo's package/target/workspace shape but omits
Rust-only edition, rust-version and crate-type fields. It does not claim
byte-for-byte Rust metadata compatibility. `metadata --no-deps` does not fetch
dependencies or invoke a compiler. Full metadata uses Cargo's existing resolver,
feature selection and graph writer with actual Sioxc execution-host platform
facts; it can fetch Git dependencies and create/update Wafer.lock. Native
`--filter-platform` is supported; unimplemented cross platforms are rejected.

Wafer discovers `.wafer/config.toml` in the current directory and its ancestors,
then merges its home configuration with Cargo's existing precedence rules.
Home defaults to `~/.wafer`; `WAFER_HOME` overrides it (relative paths resolve
from the working directory, and an empty override uses the default). Typed
configuration environment keys use `WAFER_`, e.g. `WAFER_BUILD_TARGET_DIR`.
`WAFER_TARGET_DIR` provides the direct target-directory override. `--config`
and aliases retain Cargo's validation and precedence, but aliases cannot enable
unsupported commands. Cargo home/project config and `CARGO_*` configuration
overrides do not implicitly configure Wafer. Logging/profiling and completion
use `WAFER_LOG*` and `WAFER_COMPLETE`. Profiles use `WAFER_INCREMENTAL`, not
`CARGO_INCREMENTAL`. Remaining compiler/registry environment
contracts are still unadapted behind the unsupported-operation guard.

`wafer generate-lockfile` resolves explicit path and Git dependencies through
Cargo's resolver/Git cache and writes `Wafer.lock`, using Cargo's lock encoding.
It does not read or modify a neighbouring `Cargo.lock` and needs no compiler.
`--offline`, `--locked` and `--frozen` retain Cargo's meanings. Re-running the
generator without `--locked` intentionally re-resolves dependencies. Full
metadata conservatively reuses existing pins, including with `--offline --locked`;
ordinary build execution remains disabled. Git packages use
`Wafer.toml`, including nested packages and inherited workspace metadata.
Registries and unadapted source replacements are rejected before loading them;
disabled optional transitive dependencies retain Cargo's normal feature selection
and are not fetched. Cargo's existing source walker now uses Wafer.toml/Wafer.lock
for Git selection, nested package boundaries and include/exclude rules. Its
source-level last-modified fingerprint follows those same selected files.
Sioxc dependency sidecars feed Cargo's existing encoded dependency inputs through
a compiler-specific reader. Actual native acceptance verifies literal filenames,
package/build-root rebasing, preserved symlink aliases and old-cache preservation
on malformed input. Native units now use Cargo's existing persisted fingerprint
calculation, comparison and fresh/dirty job preparation. Actual native acceptance
covers source changes, missing outputs/sidecars, corrupt records and invalidation
of an old fingerprint before a dirty job. This is API-level freshness support,
not an enabled Siox build command. The existing job queue now accepts native
runner state; production work preparation, packaging and update/fetch remain unadapted.
Sioxc sidecars do not provide checksum records, so checksum-freshness is rejected.

The compiler adapter API selects `SIOXC`, then `build.sioxc` (including
`WAFER_BUILD_SIOXC`), then `sioxc` on PATH. It queries Sioxc's own verbose
version, execution-host cfg facts, native target and default artifact filenames.
The executable is resolved against the query process's effective PATH and pinned
to an absolute path before queries or native invocations. Artifact namespaces and
persisted fingerprints include a streamed SHA256 of its contents, so embedded
core/runtime or ABI-feature changes do not hide behind an unchanged release.
External tools/libraries and execution-environment inputs still need integration
before this can be treated as a complete native build-cache identity.
Dependency filtering uses the execution host and its matching cfg facts, not
LLVM's potentially different native design triple. This requires the new
compiler-owned `-vV`/`--print` interface in the local `feat/toolchain-queries`
compiler branch; released/current older compilers are rejected rather than
treated as Rust. The shared root/dependency unit planners now accept Sioxc
platform data without Rust sysroots or crate-format probes. One build-context
planning implementation serves both compilers; `create_sioxc_bcx` exposes the
Sioxc context and the explicit unit-graph CLI prints that plan. Native acceptance
covers versioned library units, features, profiles and target filters through
Cargo's existing planner and host-unit deduplication. Rust std/MSRV handling
remains Rust-specific. Runner state and preparation accept Sioxc contexts, while
the high-level compile entry point remains Rust-only. The same job queue can
execute native work with its existing dependency ordering, jobserver and failure
handling; no second scheduler is introduced.
Configured Sioxc flags use Cargo's existing precedence: `WAFER_ENCODED_SIOXCFLAGS`
(unit-separator encoded), then `SIOXCFLAGS` (space separated), then the native
`target.<host>.sioxcflags` plus matching `target.'cfg(...)'.sioxcflags`, then
`build.sioxcflags`. Typed configuration environment keys use `WAFER_`, including
`WAFER_BUILD_SIOXCFLAGS`; arrays merge according to Cargo's existing rules.
Explicit `--target <host>` retains Cargo's separate host-artifact flag handling.
Rust flag inputs do not configure Sioxc, and selected Siox flags participate in
the shared unit identity. This is option selection, not compiler execution.
Execution and artifact/dep-info collection still need integration, so normal
build/check/test execution remains disabled.
Compiled-library formats are not emulated.

Embedded users can construct a sandboxed Wafer context with
`GlobalContext::new_wafer(shell, cwd, home)`; the existing `new` remains Cargo's
constructor. Neither requires changing the process directory or environment.

The shared `Compilation` setup can prepare either compiler's process and native
target commands. Sioxc processes receive `WAFER_*` package/unit variables and
the active `WAFER` executable; Cargo retains its `CARGO_*` environment. Rust
wrappers, sysroot search paths and documentation commands stay Rust-only.
The shared `BuildRunner` also prepares Sioxc jobservers, layouts, output caches
and collision checks. Its dry-run collects native design objects separately
from executable tests; unsupported artifacts and Rust LTO options fail before
layout creation. Callers still supply validated invocation arguments. This
does not yet prepare native work for the high-level compile entry point or
enable CLI builds.

The artifact adapter also uses Cargo's `CompilationFiles` hashes,
output-name cache and planned uplift/export paths, probing Sioxc's actual native object/test
filenames instead of inventing Rust library formats. Its package/version/mode/
feature/flag namespaces include compiler executable identity but are not a
complete toolchain or compiled-library ABI key. External-tool/input tracking,
reusable library metadata and production native work preparation/collection
remain required. Internal queue acceptance runs independently emitted native
objects and test binaries with one/two-job limits, fresh skips, dependency rebuilds,
failed-prerequisite blocking and keep-going. It also verifies Sioxc timing reports
and preserves Cargo's Rust timing report. Native acceptance verifies actual outputs
and unit namespace separation; it does not link/import the independently emitted
objects as reusable compiled libraries or claim complete toolchain freshness.

Actual compiler-query acceptance is opt-in, not a fake compiler script or an
implicit passing skip: set `WAFER_TEST_SIOXC` to that compiler binary and run
`cargo test -p cargo --test sioxc_toolchain -- --ignored` plus
`cargo test -p cargo --lib actual_sioxc_selection -- --ignored`.
Full metadata comparisons and the Cargo reference native build additionally
need `WAFER_TEST_RUSTC` pointing to the actual Rust1.90 compiler:
`cargo test -p cargo --test wafer_metadata -- --ignored`.
Both compiler paths are also required for the shared planner acceptance:
`cargo test -p cargo --lib unit_generator::tests -- --ignored`.
Dependency-sidecar acceptance additionally needs `WAFER_TEST_STD` pointing to
the actual Siox std checkout:
`cargo test -p cargo --lib fingerprint::dep_info::tests -- --ignored`.
Native shared-process acceptance uses the same compiler/std variables:
`cargo test -p cargo --lib compiler::compilation::tests -- --ignored`.
Shared runner preparation/dry-run acceptance uses those same variables:
`cargo test -p cargo --lib compiler::build_runner::tests -- --ignored`.
Shared native fingerprint acceptance uses them too:
`cargo test -p cargo --lib compiler::fingerprint::tests -- --ignored`.
Native scheduling acceptance uses the same compiler/std variables:
`cargo test -p cargo --lib compiler::job_queue::tests -- --ignored`.

Adapter validation: 23 executable CLI/Git/metadata/planning tests, four source-tracking
regressions covering both tools, two actual Sioxc toolchain integration
tests and 171 library tests (including 143 upstream Cargo tests and the explicitly
enabled native compiler-selection, shared-planning, process-setup, artifact, runner, fingerprint and scheduling tests) pass, along with
formatting and strict relevant-target Clippy. Metadata/unit graphs and features match Cargo, Git pins
are retained, and the reference Cargo binary still builds and runs a native path
dependency project. This is not Cargo-driven Siox build support. The full
upstream Cargo integration suite is not yet validated. The
local compiler query branch passed its Rust/native/query tests but its full
default and bitpack corpora each have five failures despite its864 default and843
bitpack Rust tests passing; it remains uncommitted
under the compiler house gate.

Preserve upstream LICENSE-MIT/LICENSE-APACHE and applicable third-party notices.
Cargo dependencies include MPL components; actual binary notice/source
obligations need a distribution audit. No upstream endorsement is implied.

## Upstream Cargo documentation

# Cargo

Cargo downloads your Rust project’s dependencies and compiles your project.

**To start using Cargo**, learn more at [The Cargo Book].

**To start developing Cargo itself**, read the [Cargo Contributor Guide].

[The Cargo Book]: https://doc.rust-lang.org/cargo/
[Cargo Contributor Guide]: https://rust-lang.github.io/cargo/contrib/

> The Cargo binary distributed through with Rust is maintained by the Cargo
> team for use by the wider ecosystem.
> For all other uses of this crate (as a binary or library) this is maintained
> by the Cargo team, primarily for use by Cargo and not intended for external
> use (except as a transitive dependency). This crate may make major changes to
> its APIs.

## Code Status

[![CI](https://github.com/rust-lang/cargo/actions/workflows/main.yml/badge.svg?branch=auto-cargo)](https://github.com/rust-lang/cargo/actions/workflows/main.yml)

Code documentation: <https://doc.rust-lang.org/nightly/nightly-rustc/cargo/>

## Compiling from Source

### Requirements

Cargo requires the following tools and packages to build:

* `cargo` and `rustc`
* A C compiler [for your platform](https://github.com/rust-lang/cc-rs#compile-time-requirements)
* `git` (to clone this repository)

**Other requirements:**

The following are optional based on your platform and needs.

* `pkg-config` — This is used to help locate system packages, such as `libssl` headers/libraries. This may not be required in all cases, such as using vendored OpenSSL, or on Windows.
* OpenSSL — Only needed on Unix-like systems and only if the `vendored-openssl` Cargo feature is not used.

  This requires the development headers, which can be obtained from the `libssl-dev` package on Ubuntu or `openssl-devel` with apk or yum or the `openssl` package from Homebrew on macOS.

  If using the `vendored-openssl` Cargo feature, then a static copy of OpenSSL will be built from source instead of using the system OpenSSL.
  This may require additional tools such as `perl` and `make`.

  On macOS, common installation directories from Homebrew, MacPorts, or pkgsrc will be checked. Otherwise it will fall back to `pkg-config`.

  On Windows, the system-provided Schannel will be used instead.

  LibreSSL is also supported.

**Optional system libraries:**

The build will automatically use vendored versions of the following libraries. However, if they are provided by the system and can be found with `pkg-config`, then the system libraries will be used instead:

* [`libcurl`](https://curl.se/libcurl/) — Used for network transfers.
* [`libgit2`](https://libgit2.org/) — Used for fetching git dependencies.
* [`libssh2`](https://www.libssh2.org/) — Used for SSH access to git repositories.
* [`libz`](https://zlib.net/) (AKA zlib) — Used by the above C libraries for data compression. (Rust code uses [`zlib-rs`](https://github.com/trifectatechfoundation/zlib-rs) instead.)

It is recommended to use the vendored versions as they are the versions that are tested to work with Cargo.

### Compiling

First, you'll want to check out this repository

```
git clone https://github.com/rust-lang/cargo.git
cd cargo
```

With `cargo` already installed, you can simply run:

```
cargo build --release
```

## Adding new subcommands to Cargo

Cargo is designed to be extensible with new subcommands without having to modify
Cargo itself. See [the Wiki page][third-party-subcommands] for more details and
a list of known community-developed subcommands.

[third-party-subcommands]: https://github.com/rust-lang/cargo/wiki/Third-party-cargo-subcommands


## Releases

Cargo releases coincide with Rust releases.
High level release notes are available as part of [Rust's release notes][rel].
Detailed release notes are available in the [changelog].

[rel]: https://github.com/rust-lang/rust/blob/master/RELEASES.md
[changelog]: https://doc.rust-lang.org/nightly/cargo/CHANGELOG.html

## Reporting issues

Found a bug? We'd love to know about it!

Please report all issues on the GitHub [issue tracker][issues].

[issues]: https://github.com/rust-lang/cargo/issues

## Contributing

See the **[Cargo Contributor Guide]** for a complete introduction
to contributing to Cargo.

## License

Cargo is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.

### Third party software

This product includes software developed by the OpenSSL Project
for use in the OpenSSL Toolkit (https://www.openssl.org/).

In binary form, this product includes software that is licensed under the
terms of the GNU General Public License, version 2, with a linking exception,
which can be obtained from the [upstream repository][1].

See [LICENSE-THIRD-PARTY](LICENSE-THIRD-PARTY) for details.

[1]: https://github.com/libgit2/libgit2

