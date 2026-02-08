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
intent: "Say hello"
scope: demo
risk: low

contract main() -> Int
  precondition:
    true

  postcondition:
    result == 0

  body:
    print("Hello from Covenant!")
    return 0
```

Run it:

```bash
covenant run hello.cov -c main
```

Output:
```
Hello from Covenant!
0
```

## Understanding the Structure

Every `.cov` file has a **header** and one or more **contracts**.

### File Header

```
intent: "What this file does — human-readable description"
scope: domain.module.name
risk: low
```

- **intent** — Describes the purpose. The Intent Verification Engine checks that your code matches this.
- **scope** — Dotted path for organization (like a package name).
- **risk** — `low`, `medium`, `high`, or `critical`. Higher risk levels enforce stricter verification.

### Contracts

Contracts are the building blocks of Covenant. Think of them as functions with built-in correctness guarantees:

```
contract greet(name: String) -> String
  precondition:
    name != ""

  postcondition:
    result != ""

  body:
    return "Hello, " + name + "!"
```

- **precondition** — Must be true before the body runs. If it fails, execution stops.
- **postcondition** — Must be true after the body runs. Uses `result` to refer to the return value.
- **body** — The actual implementation.

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

Contracts can call each other:

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

Covenant has 20 built-in modules. Call them with `module.method()`:

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
    -- Local Ollama
    answer = ollama.chat("What is Rust?")

    -- OpenAI (needs OPENAI_API_KEY env var)
    answer = openai.chat("Explain quantum computing")

    -- Anthropic (needs ANTHROPIC_API_KEY env var)
    answer = anthropic.chat("Write a haiku about code")

    -- Vector embeddings
    vec = ollama.embed("some text to embed")
    similarity = embeddings.cosine(vec1, vec2)
```

## Effects and Verification

Contracts can declare their effects — what they read, modify, and emit:

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

The `old()` function in postconditions refers to a value before the body executed.

Run verification:

```bash
covenant check transfer.cov
```

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
