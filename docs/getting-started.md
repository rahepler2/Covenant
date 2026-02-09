# Getting Started

## Installation

Covenant is built with Rust. You need `cargo` installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then clone and build:

```bash
git clone https://github.com/rahepler2/Covenant.git
cd Covenant
cargo build --release
```

The binary is at `target/release/covenant`. Add it to your PATH:

```bash
# Symlink (recommended — auto-updates on rebuild)
ln -sf $(pwd)/target/release/covenant /usr/local/bin/covenant

# Or copy
cp target/release/covenant /usr/local/bin/
```

Verify it works:

```bash
covenant --version
```

## Your First Contract

Create a file called `hello.cov`:

```
intent: "Say hello to the world"
scope: demo.hello
risk: low

contract main()
  body:
    print("Hello from Covenant!")
```

Run it:

```bash
covenant run hello.cov
```

Output:
```
Hello from Covenant!
null
```

That's it — 7 lines. The header (intent, scope, risk) is required, but the contract itself is just `body:` and your code. No ceremony for simple things.

## Core Concepts

Before diving deeper, let's understand the three things that make Covenant different from other languages.

### What is a Contract?

In most languages, you write **functions** — blocks of code that take input and produce output. A function makes no promises about what it will do. It could crash, return garbage, or silently corrupt your data.

In Covenant, you write **contracts**. A contract is like a function, but with built-in guarantees:

```
contract greet(name: String) -> String
  precondition:
    name != ""          -- "I promise I won't run unless name is non-empty"

  postcondition:
    result != ""        -- "I promise to return a non-empty string"

  body:
    return "Hello, " + name + "!"
```

Think of it like a legal contract: both sides have obligations. The **caller** must satisfy the preconditions (provide valid input). The **contract** must satisfy the postconditions (deliver valid output). If either side breaks the deal, execution stops immediately instead of silently producing bad results.

| Section | Purpose | Required? |
|---------|---------|-----------|
| `precondition` | What must be true **before** the code runs | No, but recommended |
| `postcondition` | What must be true **after** the code runs. Use `result` for the return value, `old(x)` for pre-execution values | No, but recommended |
| `effects` | What the code is allowed to touch (read, modify, emit) | No, but recommended |
| `body` | The actual implementation | **Yes** |
| `on_failure` | Fallback if pre/postconditions fail | No |

The more you declare, the more Covenant can verify. At higher risk levels, missing sections become compile errors.

**Shorthand forms** — for simple contracts, you don't need all the ceremony:

```
-- Expression body: one-liner for pure computations
contract square(n: Int) -> Int = n * n

-- Just a body: no pre/post/effects needed at low risk
contract say_hi()
  body:
    print("Hi!")

-- `pure` keyword: shorthand for declaring no side effects
contract add(a: Int, b: Int) -> Int
  pure
  body:
    return a + b
```

Return type (`-> Type`) is also optional — omit it when you don't need to constrain the output. Parameter types are also optional — `contract add(a, b) = a + b` works for scripting-style code.

### What is Scope?

Every `.cov` file must declare a **scope** — a namespace that says where this code belongs in your project. Think of it like a package path in Java or Go:

```
scope: finance.transfers
```

This tells Covenant (and other developers) that this file lives in the `finance` domain, specifically the `transfers` module. Scope is **enforced at compile time**:

- **Must have at least 2 segments**: `finance.transfers` is valid, `finance` alone is not.
- **Must be lowercase**: `Finance.Transfers` is an error.
- **Must relate to intent**: If your scope says `finance.transfers` but your intent says "Process sensor data", you'll get a warning.

Why does this matter? Because scope creates a traceable namespace across your entire codebase. You can use `covenant map` to see which contracts in `finance.transfers` affect `finance.ledger`, track dependencies across scopes, and understand the blast radius of changes.

```bash
-- See the full project dependency map
covenant map

-- See what a specific contract impacts
covenant map --contract transfer

-- See all contracts in a specific file
covenant map --file transfer.cov
```

### What is Risk?

Risk is **not** about how dangerous your code is. It's about **how strict the compiler should be** when checking your code:

```
risk: low       -- No warnings, maximum flexibility
risk: medium    -- No warnings, moderate verification
risk: high      -- ERRORS for missing preconditions, postconditions, effects
risk: critical  -- ERRORS for anything undeclared + strictest flow checking
```

For a hello-world demo, `risk: low` is fine — the compiler won't bother you about missing sections. For a financial transfer or medical system, `risk: high` or `risk: critical` forces you to declare everything, because the consequences of bugs are severe.

| Risk Level | Missing precondition | Missing postcondition | Missing effects | IFC checks |
|------------|---------------------|----------------------|----------------|------------|
| `low` | Silent | Silent | Silent | Basic |
| `medium` | Silent | Silent | Silent | Basic |
| `high` | **Error** | **Error** | **Error** | Full |
| `critical` | **Error** | **Error** | **Error** | Full + flow tracing |

## Variables and Types

Covenant is dynamically typed. Assign with `=`:

```
  body:
    x = 42
    name = "Alice"
    pi = 3.14
    active = true
    items = [1, 2, 3]
    person = Person(name: "Bob", age: 30)
```

### Value Types

| Type | Examples | Notes |
|------|----------|-------|
| `Int` | `42`, `-7`, `0` | 64-bit signed |
| `Float` | `3.14`, `-0.5` | 64-bit |
| `String` | `"hello"`, `"line\n"` | UTF-8, supports `\n`, `\t`, `\\`, `\"` |
| `Bool` | `true`, `false` | |
| `List` | `[1, 2, 3]`, `["a", "b"]` | Mixed types allowed |
| `Object` | `Person(name: "Alice")` | Named with keyword args |
| `Null` | `null` | |

### Type Conversion

```
  body:
    s = str(42)          -- "42"
    n = int("123")       -- 123
    f = float("3.14")    -- 3.14
    n2 = int(3.7)        -- 3 (truncates)
```

## Control Flow

### If / Else

```
  body:
    if x > 10:
      print("big")
    else:
      print("small")
```

### While Loop

```
  body:
    i = 0
    while i < 10:
      print(i)
      i = i + 1
```

### For Loop

```
  body:
    for item in items:
      print(item)

    for i in range(5):
      print(i)
```

## Lists

```
  body:
    items = ["apple", "banana", "cherry"]

    -- Indexing
    first = items[0]
    second = items[1]

    -- Length
    count = len(items)

    -- Append (returns new list)
    more = items.append("date")

    -- Concatenation
    all = [1, 2] + [3, 4]      -- [1, 2, 3, 4]

    -- Multi-line lists
    colors = [
      "red",
      "green",
      "blue"
    ]
```

## Objects

Capitalized names create objects:

```
  body:
    p = Person(name: "Alice", age: 30)
    print(p.name)     -- "Alice"
    print(p.age)      -- 30
```

## String Operations

```
  body:
    greeting = "Hello" + " " + "World"
    count = 5
    msg = "There are " + str(count) + " items"
```

## Calling Other Contracts

Contracts can call each other — just like functions calling functions:

```
contract add(a: Int, b: Int) -> Int
  precondition:
    true
  postcondition:
    result == a + b
  body:
    return a + b

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    sum = add(3, 4)
    print(sum)
    return 0
```

## Using the Standard Library

Covenant has 20 built-in modules. Call them with `module.method()` — no imports needed:

```
  body:
    -- Math
    root = math.sqrt(144)
    pi = math.pi

    -- Text
    upper = text.upper("hello")
    words = text.split("a b c")

    -- JSON
    obj = json.parse("{\"name\": \"Alice\"}")
    s = json.stringify(obj)

    -- Time
    now = time.now()

    -- File
    content = file.read("/tmp/data.txt")
    file.write("/tmp/out.txt", "hello")
```

See [Standard Library](stdlib.md) for all 20 modules.

## Using AI Modules

Covenant ships with built-in clients for LLM providers:

```
  body:
    -- Local Ollama (free, runs on your machine)
    answer = ollama.chat("What is Rust?")

    -- OpenAI (needs OPENAI_API_KEY env var)
    answer = openai.chat("Explain quantum computing")

    -- Anthropic (needs ANTHROPIC_API_KEY env var)
    answer = anthropic.chat("Write a haiku about code")

    -- Vector embeddings (for semantic search / RAG)
    vec = ollama.embed("some text to embed")
    similarity = embeddings.cosine(vec1, vec2)
```

The `embeddings` module is pure math — cosine similarity, dot products, nearest-neighbor search. The actual vectorization happens through LLM providers like `ollama.embed()` or `openai.embed()`.

## Effects and Verification

Contracts can declare their side effects — what they read, modify, and emit. The compiler verifies your code matches these declarations:

```
contract transfer(from: Account, to: Account, amount: Float) -> Bool
  precondition:
    amount > 0
    from.balance >= amount

  postcondition:
    from.balance == old(from.balance) - amount
    to.balance == old(to.balance) + amount

  effects:
    modifies: [from.balance, to.balance]
    emits: TransferCompleted
    touches_nothing_else

  body:
    from.balance = from.balance - amount
    to.balance = to.balance + amount
    emit TransferCompleted(from, to, amount)
    return true
```

The `old()` function in postconditions captures a value **before** the body ran, so you can assert how things changed.

Run verification:

```bash
covenant check transfer.cov
```

## Impact Mapping

Use `covenant map` to see what contracts affect across your codebase:

```bash
-- Full project map: all scopes, contracts, effects, dependencies
covenant map

-- What does the 'transfer' contract impact?
covenant map --contract transfer

-- What's in this file and what does it touch?
covenant map --file transfer.cov
```

This gives you a tree showing each contract's reads, writes, emits, calls, and cross-scope dependencies — so you can understand the blast radius of any change.

## Project Setup

Initialize a new project:

```bash
covenant init
```

This creates:
- `main.cov` — Starter contract
- `covenant_packages/` — Directory for local packages

## Next Steps

- [Language Reference](language-reference.md) — Complete syntax details
- [Standard Library](stdlib.md) — All 20 modules
- [Examples & Patterns](examples.md) — Real-world patterns
- [CLI Reference](cli.md) — All commands
