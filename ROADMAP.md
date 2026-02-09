# Covenant Roadmap

## v0.1.0 — Foundation (current)

The core language works end-to-end: parse, verify, compile, execute.

**What's shipped:**
- Lexer, parser, AST with source locations
- Tree-walking interpreter + bytecode VM (35 opcodes)
- 5 verification passes: IVE, fingerprinting, capability/IFC, contract verification, static type checking
- 21 stdlib modules (core + AI-age)
- Gradual type system with generics (`List<Int>`, `Map<String, Int>`)
- `async contract`, `parallel:` blocks, `await` (syntax complete, sequential execution)
- `covenant serve` — contracts as HTTP API endpoints
- `db` module — SQLite persistence
- Auto-escalation with suggested fixes
- 131 Python tests, 18 example programs
- Behavioral fingerprinting + SHA-256 intent hashing

**What's missing:** Rust test suite, true concurrency, error handling beyond `on_failure`, no package registry, no editor support.

---

## v0.2.0 — Reliability

Make it trustworthy. Add a Rust test suite, structured error handling, and harden edge cases.

- [ ] Rust unit + integration tests (target: 200+ tests covering lexer, parser, interpreter, VM, verification)
- [ ] `try/catch/finally` syntax for structured error handling
- [ ] Error type system: typed errors with pattern matching in catch blocks
- [ ] `Result<T, E>` and `Option<T>` as first-class types
- [ ] Fix known false positives (W001 on abstract operations, E003 on stdlib calls)
- [ ] Property-based fuzzing for parser (malformed input doesn't panic)
- [ ] CI pipeline (GitHub Actions: build + test + lint)
- [ ] Benchmark suite (parse/verify/run times for standard programs)

---

## v0.3.0 — Concurrency

Make `async` and `parallel:` real. This is the biggest technical leap.

- [ ] Async runtime with non-blocking I/O (tokio or custom event loop)
- [ ] True concurrent execution for `parallel:` blocks (thread pool or green threads)
- [ ] `channel` type for inter-contract communication
- [ ] `select` statement for waiting on multiple async sources
- [ ] `stream` iteration for processing async sequences
- [ ] Rate limiting and backpressure for AI API calls
- [ ] Concurrency-aware verification (race condition detection in `effects`)
- [ ] Update `covenant serve` to handle concurrent requests

---

## v0.4.0 — Developer Experience

Make it pleasant to use day-to-day.

- [ ] LSP server (completions, diagnostics, go-to-definition, hover)
- [ ] VS Code extension
- [ ] Hot reload for `covenant serve` (watch .cov files, restart on change)
- [ ] REPL (`covenant repl` — interactive contract execution)
- [ ] `covenant fmt` — auto-formatter
- [ ] `covenant test` — built-in test runner (test contracts with assertions)
- [ ] Source maps for bytecode (error messages point to .cov line numbers)
- [ ] Colored terminal output for diagnostics

---

## v0.5.0 — Ecosystem

Make it shareable. Package management, versioning, and community infrastructure.

- [ ] `covenant.toml` — project manifest (dependencies, metadata, build config)
- [ ] Package registry (hosted or decentralized)
- [ ] Semantic versioning for packages
- [ ] Dependency resolution and lock files
- [ ] `covenant publish` — upload packages to registry
- [ ] `covenant update` — update dependencies
- [ ] Standard package template with tests and documentation
- [ ] Cross-file imports with namespace scoping

---

## v0.6.0 — Advanced Types

Make the type system powerful enough for large applications.

- [ ] Traits / interfaces (`trait Serializable { ... }`)
- [ ] `impl` blocks for adding methods to types
- [ ] Pattern matching (`match value: ...`)
- [ ] Algebraic data types / tagged unions (`type Shape = Circle(r) | Rect(w, h)`)
- [ ] Type aliases (`type UserId = Int`)
- [ ] Const contracts (compile-time evaluation for pure contracts)
- [ ] Generic contracts (`contract map<T, U>(items: List<T>, f: T -> U) -> List<U>`)

---

## v0.7.0 — Performance & Portability

Make it fast and run anywhere.

- [ ] WASM compilation target (run Covenant in the browser)
- [ ] JIT compilation for hot contracts (optional, via cranelift or similar)
- [ ] Bytecode optimization passes (constant folding, dead code elimination)
- [ ] Memory pooling for VM (reduce allocations)
- [ ] Benchmark against Python/JS/Lua for equivalent programs
- [ ] Cross-compilation for ARM/Linux/macOS/Windows

---

## v0.8.0 — Production Readiness

Make it deployable with confidence.

- [ ] Structured logging (`covenant serve --log-format json`)
- [ ] Metrics endpoint (`/metrics` for Prometheus)
- [ ] Health checks (`/health` endpoint)
- [ ] Graceful shutdown
- [ ] Request tracing (OpenTelemetry)
- [ ] Rate limiting middleware
- [ ] TLS support for `covenant serve`
- [ ] Docker image published to registry
- [ ] Systemd service template

---

## v1.0.0 — Stable

The language is stable. Breaking changes require a new major version.

- [ ] Language specification document (formal grammar, semantics)
- [ ] API stability guarantee (syntax, stdlib, bytecode format)
- [ ] Backwards compatibility policy
- [ ] Migration tooling for breaking changes
- [ ] Security audit (third-party review of parser, runtime, serve)
- [ ] Performance regression tests
- [ ] Published documentation site
- [ ] Multiple real-world applications in production

---

## Version Summary

| Version | Theme | Key Deliverable |
|---------|-------|----------------|
| **0.1.0** | Foundation | Core language + verification + VM + stdlib |
| **0.2.0** | Reliability | Test suite + error handling + CI |
| **0.3.0** | Concurrency | Real async/parallel + channels |
| **0.4.0** | DX | LSP + hot reload + REPL + formatter |
| **0.5.0** | Ecosystem | Package registry + dependency management |
| **0.6.0** | Types | Traits + pattern matching + generics |
| **0.7.0** | Performance | WASM + JIT + optimization |
| **0.8.0** | Production | Logging + metrics + TLS + Docker |
| **1.0.0** | Stable | Spec + stability guarantee + security audit |
