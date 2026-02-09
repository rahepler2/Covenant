# Covenant

An AI-first programming language where every function is a verified contract.

Covenant is designed for a world where AI agents write code and humans audit it. Every function is a **contract** with compiler-enforced preconditions, postconditions, and declared effects. Every file declares its **intent**, **scope**, and **risk level**. Every data type carries **information flow constraints**. The compiler catches semantic drift, undeclared side effects, and type mismatches before anything runs.

## Quick Start

```bash
# Build from source (Rust 1.70+)
cargo build --release

# Run a contract
covenant run examples/concise.cov

# Verify a file (intent, effects, types, capabilities)
covenant check examples/transfer.cov

# Start an API server (contracts become endpoints)
covenant serve --port 8080 --static-dir static/
```

## Verbosity Scales With Risk

Covenant has one rule: **the more your code impacts the outside world, the more you must declare**. Pure helpers are one-liners. Functions that mutate state require contracts. The language enforces this automatically.

### Level 1: Pure One-Liners

No ceremony. Expression body, done.

```covenant
contract tax_rate(country: String) -> Float = 0.20
contract net(gross: Float, rate: Float) -> Float = gross * (1.0 - rate)
contract flag(amount: Float, limit: Float) -> Bool = amount > limit
```

### Level 2: Pure Helpers With Logic

More complex, but still no side effects. `pure` is shorthand for `effects: touches_nothing_else`.

```covenant
contract classify_expense(amount: Float) -> String
  pure
  body:
    if amount > 10000.0:
      return "HIGH"
    if amount > 1000.0:
      return "MEDIUM"
    return "LOW"
```

### Level 3: Side Effects Require Declaration

This contract writes to a database and emits an event. Covenant **requires** the `effects` block. Without it, you get a compile error with the exact fix:

```
ERROR W005: contract 'save_record' has external side effects
  (mutates db; emits RecordSaved) but no effects: block. Add:
  effects:
    modifies [db]
    emits RecordSaved
  Or mark the contract `pure` if it should have no side effects.
```

The verified version:

```covenant
contract analyze_expenses() -> Int
  precondition:
    true
  postcondition:
    result >= 0
  effects:
    modifies [audit.log]
    emits AuditComplete
  body:
    -- ... process expenses, flag anomalies ...
    fingerprint = crypto.sha256(str(total) + str(flagged))
    emit AuditComplete(total, flagged, fingerprint)
    return flagged
```

### Level 4: High-Risk Requires Everything

At `risk: high`, preconditions, postconditions, effects, and `on_failure` are all required:

```covenant
intent: "Transfer funds between accounts"
scope: finance.transfers
risk: high

contract transfer(from: Account, to: Account, amount: Currency) -> TransferResult
  precondition:
    from.balance >= amount
    amount > Currency(0)
  postcondition:
    from.balance == old(from.balance) - amount
    to.balance == old(to.balance) + amount
  effects:
    modifies [from.balance, to.balance]
    emits TransferEvent
    touches_nothing_else
  body:
    hold = ledger.escrow(from, amount)
    ledger.deposit(to, hold)
    emit TransferEvent(from, to, amount, time.now())
    return TransferResult.success(receipt: hold.receipt)
  on_failure:
    ledger.rollback(hold)
    return TransferResult.insufficient_funds()
```

**Try the flagship demo** to see this in action:
```bash
covenant check examples/flagship.cov        # Verify
covenant fingerprint examples/flagship.cov   # See behavioral fingerprints
covenant run examples/flagship.cov           # Execute via bytecode VM
```

## Type System

Gradual typing — add types when you want safety, omit them when scripting.

```covenant
-- Untyped (inferred as Any)
contract double(n) = n * 2

-- Fully typed with generics
contract process(items: List<Int>, threshold: Float) -> List<Int>
  body:
    result = []
    for item in items:
      if item > threshold:
        result = result + [item]
    return result
```

Types are enforced at runtime (both interpreter and VM) and checked statically by `covenant check` (T001-T004 codes).

## Async and Parallelism

```covenant
-- Async contracts
async contract fetch_data(url: String) -> Object
  body:
    response = await web.get(url)
    return response.json()

-- Parallel execution blocks
contract process_all()
  body:
    parallel:
      data = web.get("https://api.example.com/data")
      config = file.read("config.json")
      status = db.query(conn, "SELECT count(*) FROM jobs")
    -- All three run concurrently, results available after the block
```

## Standard Library

21 built-in modules — no installation needed.

### Tier 1 — Core

| Module | Key Methods | Description |
|--------|------------|-------------|
| **math** | `sqrt`, `pow`, `sin`, `cos`, `log`, `pi`, `random` | Math functions and constants |
| **text** | `split`, `join`, `replace`, `matches`, `find_all`, `trim`, `upper` | String processing with regex |
| **json** | `parse`, `stringify` | JSON serialization |
| **file** | `read`, `write`, `append`, `exists`, `lines`, `delete` | File I/O (10MB limit) |
| **crypto** | `sha256`, `hmac` | Cryptographic hashing |
| **time** | `now`, `timestamp`, `sleep`, `elapsed`, `format` | Time utilities |
| **env** | `get`, `set`, `has`, `all` | Environment variables |
| **data** | `frame`, `from_records`, `read_csv` | DataFrame with `filter`, `sort_by`, `mean`, `sum`, `head`, `show` |
| **web** | `get`, `post`, `put`, `delete` | HTTP client with `HttpResponse` |
| **ai** | `prompt`, `classify`, `extract`, `summarize` | High-level AI operations |
| **db** | `open`, `execute`, `query`, `tables`, `close` | SQLite database (parameterized queries, JSON results) |

### Tier 2 — AI-Age

| Module | Description |
|--------|-------------|
| **http** | Full HTTP client (timeout, auth, JSON body) |
| **anthropic** | Claude API (chat, models) |
| **openai** | OpenAI API (chat, embed, image, models) |
| **ollama** | Local LLMs (chat, generate, embed, pull) |
| **grok** | xAI Grok API |
| **embeddings** | Vector math (cosine, dot, nearest, normalize) |
| **prompts** | Prompt engineering (template, few_shot, messages) |
| **guardrails** | Output validation (PII detection, schema validation, sanitize) |
| **mcp** | Model Context Protocol (connect, list_tools, call_tool) |
| **mcpx** | MCP extensions (router, chain, parallel, fallback) |

## Full-Stack Applications

Covenant can serve contracts as HTTP API endpoints. Each contract becomes a route with automatic input validation (preconditions) and output guarantees (postconditions).

```bash
# Start an API server — contracts become endpoints
covenant serve --port 8080 --static-dir static/

# Routes are derived from contract names:
#   GET  /api/list_users     (get_/list_ prefix = GET)
#   POST /api/create_user    (everything else = POST)
#   GET  /api/get_user?id=1  (query params for GET)
```

### Database

```covenant
contract init()
  effects:
    modifies [database]
  body:
    conn = db.open("app.db")
    db.execute(conn, "CREATE TABLE IF NOT EXISTS users (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL
    )")
    return true

contract list_users() -> List
  body:
    conn = db.open("app.db")
    return db.query(conn, "SELECT * FROM users")

contract create_user(name: String)
  precondition:
    name != ""
  body:
    conn = db.open("app.db")
    db.execute(conn, "INSERT INTO users (name) VALUES (?)", [name])
    return true
```

See `examples/fullstack/` for a complete task manager with frontend and `docs/app-architecture.md` for the full guide.

## CLI Reference

```
covenant run <file.cov>                    Execute a contract (VM by default)
covenant run <file.cov> -c <name>          Execute a specific contract
covenant run <file.cov> --arg k=v          Pass arguments
covenant run <file.cov> --interpret        Use tree-walking interpreter
covenant check <file.cov>                  Run all verification passes
covenant fingerprint <file.cov>            Show behavioral fingerprints + intent hashes
covenant serve [files...] --port 8080      Start HTTP server (contracts → endpoints)
covenant serve --static-dir static/        Serve static files alongside API
covenant build <file.cov> -o out.covc      Compile to bytecode
covenant exec <file.covc>                  Run pre-compiled bytecode
covenant disasm <file.cov>                 Show bytecode disassembly
covenant map [dir] [--contract name]       Show impact map of contract dependencies
covenant init                              Initialize a new project
covenant add <package> [--global]          Install a package
covenant packages                          List available modules
covenant parse <file.cov>                  Display AST
covenant tokenize <file.cov>               Display token stream
```

## Verification System

`covenant check` runs five verification passes:

**Intent Verification Engine (IVE)**
- E001-E005: Undeclared mutations, effect violations, missing body, undeclared emits
- W001-W008: Soundness, missing sections, relevance, achievability, scope
- I001-I002: Recursion and deep nesting detection

**Behavioral Fingerprinting**
- Extracts reads, mutations, calls, events, `old()` refs, capability checks
- SHA-256 intent hash binds declared intent to actual behavior

**Capability Type System / IFC**
- F001-F006: Flow violations, permission denied, context required, grant-deny conflicts

**Contract Verification**
- V001-V005: Missing returns, dead code, missing on_failure, shared state access

**Static Type Checking**
- T001-T004: Argument type mismatch, return type mismatch, operator mismatch, wrong arg count

**Auto-Escalation**: Verification is proportional to risk. At `high`/`critical` risk, missing preconditions/postconditions/effects are errors. At **any** risk level, contracts with external side effects but no `effects` block are rejected — with a suggested fix showing the exact declaration needed.

## Bytecode VM

Covenant includes a bytecode compiler and stack-based virtual machine:

- 35-opcode instruction set (~12 bytes per instruction, L1 cache friendly)
- AST-to-bytecode compiler with backpatching for jumps
- Binary `.covc` format with serialization/deserialization
- `covenant run` uses the VM by default; `--interpret` falls back to the tree-walker

```bash
covenant build app.cov -o app.covc    # Compile
covenant exec app.covc -c main        # Run bytecode
covenant disasm app.cov                # Inspect
```

## Project Structure

```
src/
  lexer/          Tokenizer (2-space indent, tabs are errors)
  parser/         Recursive descent parser
  ast/            AST with source locations on every node
  verify/
    fingerprint   Behavioral fingerprinting
    checker       IVE verification (E/W/I/S codes)
    hasher        SHA-256 intent hashing
    capability    Capability type system / IFC (F codes)
    contract_verify  Contract verification (V codes)
    type_check    Static type inference and checking (T codes)
  runtime/
    mod           Tree-walking interpreter
    stdlib/       Standard library (21 modules)
  vm/
    opcodes       35-opcode instruction set
    compiler      AST → bytecode compiler
    machine       Stack-based VM
    bytecode      Binary .covc format
  serve.rs        HTTP server (contracts → API endpoints)
  main.rs         CLI (clap)
  packages/       Package resolution

examples/         Example .cov programs
docs/             Language reference, stdlib docs, architecture guide
```

## Security

- **Checked arithmetic** — integer overflow is a RuntimeError
- **Call depth limit** — max recursion depth of 256
- **Parser depth limit** — max expression nesting of 256
- **Range cap** — `range()` limited to 10M elements
- **File size limit** — source files capped at 10MB
- **Loop limit** — while loops capped at 1M iterations
- **Sleep cap** — `time.sleep()` max 60 seconds
- **SQL parameterization** — `db` module uses `?` placeholders

## Performance

- Full stdlib demo (8 contracts, 10 modules): **~15ms** (release build)
- Parse + verify + execute a single contract: **<5ms**
- Release binary: optimized with LTO

## Roadmap

- [ ] True concurrent execution for `parallel:` blocks (currently sequential)
- [ ] Async runtime with real non-blocking I/O
- [ ] Package registry and versioning
- [ ] LSP server for editor integration
- [ ] WASM compilation target
- [ ] Structured error types and try/catch
- [ ] Trait/interface system
- [ ] Hot reload for `covenant serve`

## License

See [LICENSE](LICENSE) for details.
