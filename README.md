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
See TODO.md for unfinished compiler orchestration and build-cache work.

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
is used for identifier validation only. Real builds will invoke `sioxc` with
explicit module inputs rather than copy packages or read manifests in sioxc.

Validation: 16 CLI/package tests include eight real-Git scenarios; strict
Clippy and formatting checks pass. These verify dependency handling, not
native compiler builds/tests, which are still under development.

