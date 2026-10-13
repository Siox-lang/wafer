# Wafer implementation

- Publish/integrate compiler prerequisites (`--module`, `--emit test-plan`,
  `--test-source`) once the compiler commit gate/owner decision is resolved.
- Compiler dependency reporting before enabling caching by default. Current
  opt-in cache covers declared inputs; do not claim undeclared external reads
  or mutable system libraries are automatically tracked.
- Final integration/acceptance against a committed compatible compiler and run
  the configured native CI. Local 19-test + nine-native-case gate is green,
  but compiler prerequisite publication/CI execution is not yet verified.
- Later: registry server/publishing, compiled die metadata, aliases and
  version-isolated namespaces once the compiler supports those contracts.

