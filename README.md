# Covenant

An intent-first programming language for the AI era.

Covenant is designed from first principles for a world where AI agents are the primary code authors and humans are the primary code auditors. Every function is a **contract** with compiler-enforced preconditions, postconditions, and declared effects. Every data type carries **information flow constraints** that the compiler enforces. Every execution produces a **cryptographically chained audit log**.

## Quick Start

```bash
pip install -e ".[dev]"

# Parse a .cov file and display its AST
covenant parse examples/transfer.cov

# Run Stage 1 structural verification
covenant check examples/transfer.cov

# Display the raw token stream
covenant tokenize examples/transfer.cov

# Run the test suite
pytest
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

## Key Language Features

- **Intent declarations** -- compiler-hashed and bound to behavioral profiles; semantic drift is caught structurally
- **Contract-based functions** -- mandatory preconditions, postconditions, and effect declarations
- **Capability-based types** -- information flow control at the type level (`[pii, no_log, encrypt_at_rest]`)
- **`touches_nothing_else`** -- compiler-verified assertion that a contract has no undeclared side effects
- **`old()` expressions** -- reference pre-execution state in postconditions
- **`has` expressions** -- capability checks as first-class expressions
- **Risk levels** -- `low`, `medium`, `high`, `critical` trigger different verification and deployment pipelines
- **2-space indentation** -- tabs are a lexer error; fixed width eliminates ambiguity

## Project Structure

```
src/covenant/
  lexer/      -- Tokenizer with indentation-sensitive scanning
  parser/     -- Hand-written recursive descent parser
  ast/        -- Immutable AST node dataclasses
  compiler/   -- Three-stage compilation pipeline (planned)
  runtime/    -- Capability-enforcing execution environment (planned)
  types/      -- Capability-based type system with IFC (planned)
  cli.py      -- Command-line interface

examples/     -- Example .cov programs
tests/        -- Test suite (63 tests)
```

## Implementation Status

**Phase 1 (Complete):** Lexer, Parser, AST -- the full language syntax can be tokenized, parsed, and represented as an immutable AST with source location tracking and source hashing for audit provenance.

**Phase 2 (Planned):** Intent Verification Engine -- behavioral fingerprinting and intent-to-code consistency checks.

**Phase 3 (Planned):** Capability Type System -- information flow control, security labels, and flow constraint enforcement.

**Phase 4 (Planned):** Contract Verification -- precondition/postcondition satisfiability, effect verification, mutation graph analysis.
