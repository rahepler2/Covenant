# Covenant

An intent-first programming language for the AI era.

Covenant is designed from first principles for a world where AI agents are the primary code authors and humans are the primary code auditors. Every function is a **contract** with compiler-enforced preconditions, postconditions, and declared effects. Every data type carries **information flow constraints** that the compiler enforces. Every execution produces a **cryptographically chained audit log**.

## Quick Start

### Option 1: Pre-built Binary

Download the latest release binary for your platform, then:

```bash
# Parse a .cov file and display its AST
covenant parse examples/transfer.cov

# Run full verification (IVE + capability checks + contract verification)
covenant check examples/transfer.cov

# Execute a contract
covenant run examples/stdlib_demo.cov -c main

# Execute with arguments
covenant run examples/fibonacci.cov -c fibonacci --arg n=10

# Display the raw token stream
covenant tokenize examples/transfer.cov

# Show behavioral fingerprints and intent hashes
covenant fingerprint examples/transfer.cov
```

### Option 2: Build from Source (Rust)

Requires Rust 1.70+ and Cargo.

```bash
# Clone the repository
git clone https://github.com/rahepler2/Covenant.git
cd Covenant

# Build in release mode
cargo build --release

# The binary is at target/release/covenant
target/release/covenant run examples/stdlib_demo.cov -c main

# Or run directly via cargo
cargo run -- run examples/stdlib_demo.cov -c main
```

### Option 3: Docker

```dockerfile
FROM rust:1.77-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/covenant /usr/local/bin/covenant
COPY examples/ /app/examples/
WORKDIR /app
ENTRYPOINT ["covenant"]
```

```bash
# Build the Docker image
docker build -t covenant .

# Run a contract
docker run --rm covenant run examples/stdlib_demo.cov -c main

# Run verification
docker run --rm covenant check examples/transfer.cov

# Mount your own .cov files
docker run --rm -v $(pwd)/my_project:/app/my_project covenant run my_project/app.cov -c main
```

## Example: Fund Transfer Contract

```covenant
intent: "Transfer funds between two accounts if sufficient balance exists"
scope: finance.transfers
risk: high
requires: [auth.verified, ledger.write_access]

contract transfer(from: Account, to: Account, amount: Currency) -> TransferResult
  precondition:
    from.balance >= amount
    from.owner has auth.current_session
    amount > Currency(0)

  postcondition:
    from.balance == old(from.balance) - amount
    to.balance == old(to.balance) + amount
    ledger.sum() == old(ledger.sum())

  effects:
    modifies [from.balance, to.balance]
    emits TransferEvent
    touches_nothing_else

  body:
    hold = ledger.escrow(from, amount)
    ledger.deposit(to, hold)
    emit TransferEvent(from, to, amount, timestamp.now())
    return TransferResult.success(receipt: hold.receipt)

  on_failure:
    ledger.rollback(hold)
    return TransferResult.insufficient_funds()
```

## Standard Library

Covenant ships with 10 built-in modules designed for AI-first development:

| Module | Functions | Description |
|--------|-----------|-------------|
| **web** | `get`, `post`, `put`, `delete` | HTTP client — returns `HttpResponse` with `.json()`, `.text()` |
| **data** | `frame`, `from_records`, `read_csv` | DataFrame with `filter`, `sort_by`, `group_by`, `select`, `sum`, `mean`, `head`, `tail`, `show` |
| **json** | `parse`, `stringify` | JSON serialization/deserialization |
| **file** | `read`, `write`, `append`, `exists`, `lines`, `delete` | File I/O with size limits |
| **ai** | `prompt`, `classify`, `extract`, `summarize` | LLM integration (Anthropic/OpenAI) |
| **crypto** | `sha256`, `hmac` | Cryptographic hashing |
| **time** | `now`, `timestamp`, `sleep`, `elapsed`, `format` | Time utilities |
| **math** | `sqrt`, `pow`, `floor`, `ceil`, `round`, `sin`, `cos`, `log`, `pi`, `e`, `random` | Extended math |
| **text** | `split`, `join`, `replace`, `matches`, `find_all`, `trim`, `upper`, `lower`, `slice` | String processing with regex |
| **env** | `get`, `set`, `has`, `all` | Environment variables |

### Stdlib Example

```covenant
intent: "Analyze sales data and generate report"
scope: analytics.sales
risk: low

contract analyze_sales() -> Int
  precondition:
    true

  postcondition:
    result >= 0

  effects:
    touches_nothing_else

  body:
    df = data.frame(columns: ["product", "revenue", "units"], rows: [["Widget", 50000, 1200], ["Gadget", 75000, 800], ["Doohickey", 30000, 2000]])
    df.show()

    total = df.sum("revenue")
    avg = df.mean("revenue")
    print("Total revenue:", total)
    print("Average revenue:", avg)

    top = df.sort_by("revenue", "desc")
    top.head(1).show()

    hash = crypto.sha256(json.stringify(df))
    print("Data fingerprint:", hash)

    return df.count()
```

## Key Language Features

- **Intent declarations** -- compiler-hashed and bound to behavioral profiles; semantic drift is caught structurally
- **Contract-based functions** -- mandatory preconditions, postconditions, and effect declarations
- **Capability-based types** -- information flow control at the type level (`[pii, no_log, encrypt_at_rest]`)
- **`touches_nothing_else`** -- compiler-verified assertion that a contract has no undeclared side effects
- **`old()` expressions** -- reference pre-execution state in postconditions
- **`has` expressions** -- capability checks as first-class expressions
- **Risk levels** -- `low`, `medium`, `high`, `critical` trigger different verification and deployment pipelines
- **2-space indentation** -- tabs are a lexer error; fixed width eliminates ambiguity

## CLI Reference

```
covenant tokenize <file.cov>        Display the token stream
covenant parse <file.cov>           Parse and display the AST
covenant check <file.cov>           Run all verification passes
covenant fingerprint <file.cov>     Show behavioral fingerprints + intent hashes
covenant run <file.cov>             Execute the first contract
covenant run <file.cov> -c <name>   Execute a specific contract
covenant run <file.cov> --arg k=v   Pass arguments to the contract
```

## Verification System

Covenant runs five verification passes on every `check`:

**Phase 1 -- Intent Verification Engine (IVE)**
- E001-E005: Undeclared mutations, effect violations, missing body, undeclared emits
- W001-W008: Soundness, missing sections, relevance, achievability, scope warnings
- I001-I002: Recursion and deep nesting info

**Phase 2 -- Behavioral Fingerprinting**
- Extracts reads, mutations, calls, events, old() refs, capability checks
- Detects branching, looping, recursion patterns
- SHA-256 intent hash binds declared intent to behavioral fingerprint

**Phase 3 -- Capability Type System / IFC**
- F001: Information flow violation (tainted data flows to restricted sink)
- F002: Permission denied (body accesses denied data)
- F003: Access not granted (body reads data not covered by grants)
- F004: Context required (requires_context not satisfied)
- F005: Capability not declared
- F006: Grant-deny conflict

**Phase 4 -- Contract Verification**
- V001: Not all code paths return a value
- V002: Dead code after return
- V003: High/critical risk contract missing on_failure handler
- V004: Postcondition references result but body may not return
- V005: Shared state access without declaration

## Project Structure

```
src/
  lexer/          Tokenizer with indentation-sensitive scanning
  parser/         Hand-written recursive descent parser
  ast/            AST enums and structs with source locations
  verify/
    fingerprint   Behavioral fingerprinting
    checker       IVE verification rules (E/W/I codes)
    hasher        SHA-256 intent hashing
    capability    Capability type system and IFC (F codes)
    contract_verify  Contract verification (V codes)
  runtime/
    mod           Tree-walking interpreter with scope stack
    stdlib/       Standard library (10 modules)
  main.rs         CLI entry point (clap)

examples/         Example .cov programs
src/covenant/     Python reference implementation (131 tests)
```

## Security

The runtime is hardened against common abuse patterns:

- **Checked arithmetic** -- all integer operations use checked_add/sub/mul/div; overflow is a RuntimeError
- **Call depth limit** -- maximum recursion depth of 256
- **Parser depth limit** -- maximum expression nesting of 256
- **Range cap** -- `range()` limited to 10 million elements
- **File size limit** -- source files capped at 10MB
- **Loop limit** -- while loops capped at 1 million iterations
- **Time.sleep cap** -- maximum 60 seconds

## Performance

The Rust toolchain is fast:

- Full stdlib demo (8 contracts, 10 modules): **~15ms** (release build)
- Parse + verify + execute a single contract: **<5ms**
- Release binary size: optimized with LTO

## License

See [LICENSE](LICENSE) for details.
