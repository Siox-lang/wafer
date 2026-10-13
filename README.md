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
```

The root source declares `module spi_controller;`. Module keys are complete
Siox namespace paths; the compiler checks their declarations. Package names
may contain hyphens, which become underscores in the default root namespace.
Paths in `[lib]`/`[modules]` stay within their die; dependency paths may point
to sibling dies. Packages and compile-time source operations must be trusted.

Initial commands: `wafer new <directory>`, `wafer init [directory]`,
`wafer metadata [--manifest-path <Die.toml>] [--locked]` and `wafer fetch`.
Metadata is JSON on stdout for tools such as the LSP; status/errors go to stderr.
See TODO.md for unfinished Git, compiler orchestration and cache work.

Rust 1.90 or newer is required. The compiler Git dependency disables LLVM and
is used for identifier validation only. Real builds will invoke `sioxc` with
explicit module inputs rather than copy packages or read manifests in sioxc.

