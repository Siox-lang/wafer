# Wafer — isolated Cargo fork

This checkout adapts Cargo for Siox, based on upstream commit
`840b83a10fb0e039a83f4d70ad032892c287570a` (Rust1.90 branch). It is separate
from the existing standalone Wafer implementation and is not a finished release.
The Rust build package/library remain named cargo to keep upstream integration
and its locked dependency graph intact. The new executable is named wafer.

Siox projects use Wafer.toml and library names directly as namespaces. Bootstrap
commands are `wafer locate-project` and `wafer metadata --no-deps --format-version 1`.
Source targets follow Cargo's layout with `.siox` instead of `.rs`: `src/lib.siox`,
`src/main.siox`, and files or `<name>/main.siox` below `src/bin/`, `examples/`,
`tests/` and `benches/`. Explicit target paths and `auto* = false` overrides
are supported. `[lib] name = "vendor"` sets the single library namespace.
Rust legacy paths and automatic `build.rs` discovery do not apply to Wafer;
explicit build scripts are rejected until their adapter exists. Compilation,
dependency resolution and mutation/registry commands still need adaptation. They are not implied
by the inherited Cargo help/API or resolver code. See TODO.md and chat.md.

Bootstrap metadata uses Cargo's package/target/workspace shape but omits
Rust-only edition, rust-version and crate-type fields. It does not claim
byte-for-byte Rust metadata compatibility. The current read-only commands do
not fetch dependencies or invoke a compiler.

Wafer discovers `.wafer/config.toml` in the current directory and its ancestors,
then merges its home configuration with Cargo's existing precedence rules.
Home defaults to `~/.wafer`; `WAFER_HOME` overrides it (relative paths resolve
from the working directory, and an empty override uses the default). Typed
configuration environment keys use `WAFER_`, e.g. `WAFER_BUILD_TARGET_DIR`.
`WAFER_TARGET_DIR` provides the direct target-directory override. `--config`
and aliases retain Cargo's validation and precedence, but aliases cannot enable
unsupported commands. Cargo home/project config and `CARGO_*` configuration
overrides do not implicitly configure Wafer. Logging/profiling and completion
use `WAFER_LOG*` and `WAFER_COMPLETE`. Remaining compiler/registry environment
contracts are still unadapted behind the unsupported-operation guard.

Bootstrap validation: 11 executable CLI tests and 145 library tests (including
143 upstream Cargo tests) pass, along with formatting and strict relevant-target Clippy. The full
upstream integration suite and Sioxc compilation are not yet validated.

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

