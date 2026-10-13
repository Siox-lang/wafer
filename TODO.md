# Wafer implementation

- Publish/integrate compiler prerequisites (`--module`, `--emit test-plan`,
  `--test-source`) once the compiler commit gate/owner decision is resolved.
- Content-based build caching and safe clean, with lock/concurrency protection.
- Project settings usable by the separate LSP without embedding package
  discovery or build execution in the server.
- Full acceptance with actual local/Git dies and native designs/tests, CI,
  documentation and scoped commits. Do not call foundations the whole goal.
- Later: registry server/publishing, compiled die metadata, aliases and
  version-isolated namespaces once the compiler supports those contracts.

