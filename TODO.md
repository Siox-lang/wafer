# Wafer implementation

- Publish/integrate compiler prerequisites (`--module`, `--emit test-plan`,
  `--test-source`) once the compiler commit gate/owner decision is resolved.
- Compiler dependency reporting before enabling caching by default. Current
  opt-in cache covers declared inputs; do not claim undeclared external reads
  or mutable system libraries are automatically tracked.
- Project settings usable by the separate LSP without embedding package
  discovery or build execution in the server.
- Full acceptance with actual local/Git dies and native designs/tests, CI,
  documentation and scoped commits. Do not call foundations the whole goal.
- Later: registry server/publishing, compiled die metadata, aliases and
  version-isolated namespaces once the compiler supports those contracts.

