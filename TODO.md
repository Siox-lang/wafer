# Wafer implementation

- Git dies: pinned commits, transitive dependencies, offline cache and update;
  preserve path/version validation and reject unsafe Git transports/options.
- Compiler orchestration: check/build/test using the explicit module-input
  boundary; native test filtering and waveform output; propagate failures.
- Content-based build caching and safe clean, with lock/concurrency protection.
- Project settings usable by the separate LSP without embedding package
  discovery or build execution in the server.
- Full acceptance with actual local/Git dies and native designs/tests, CI,
  documentation and scoped commits. Do not call foundations the whole goal.
- Later: registry server/publishing, compiled die metadata, aliases and
  version-isolated namespaces once the compiler supports those contracts.

