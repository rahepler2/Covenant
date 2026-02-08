# Covenant Documentation

Covenant is a programming language designed for the age of AI. It uses **contracts** instead of functions, with built-in preconditions, postconditions, and intent verification.

## Guides

- **[Getting Started](getting-started.md)** — Install, write your first contract, run it
- **[Language Reference](language-reference.md)** — Complete syntax, types, operators, control flow
- **[Standard Library](stdlib.md)** — All 20 built-in modules with examples
- **[Examples & Patterns](examples.md)** — RAG systems, API clients, data pipelines, and more
- **[CLI Reference](cli.md)** — All `covenant` commands

## Quick Example

```
intent: "Calculate the factorial of a number"
scope: math.factorial
risk: low

contract factorial(n: Int) -> Int
  precondition:
    n >= 0

  postcondition:
    result >= 1

  body:
    if n <= 1:
      return 1
    return n * factorial(n - 1)

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    print(factorial(10))
    return 0
```

```bash
covenant run factorial.cov -c main
```

## Install

```bash
# From source (requires Rust)
git clone https://github.com/rahepler2/Covenant.git
cd Covenant
cargo build --release

# Add to PATH
ln -sf $(pwd)/target/release/covenant /usr/local/bin/covenant
```
