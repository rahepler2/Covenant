# Language Reference

## Syntax Basics

- **Indentation**: 2 spaces. Tabs are syntax errors.
- **Comments**: `-- this is a comment`
- **Strings**: `"double quoted"` with escapes `\n`, `\t`, `\\`, `\"`
- **Multi-line expressions**: Lists `[...]` and function calls `func(...)` can span multiple lines.

## File Structure

```
-- Optional comments

intent: "Description of what this file does"
scope: domain.module
risk: low | medium | high | critical
requires: [capability1, capability2]

use modulename
use modulename as alias

type MyType(BaseType):
  fields:
    name: String
    value: Int [sensitive]
  flow_constraints:
    never_flows_to: [logging, external_api]

shared counter: Int:
  access: read-write
  isolation: exclusive
  audit: log_writes

contract name(param: Type) -> ReturnType
  ...
```

### Header Fields

| Field | Required | Description |
|-------|----------|-------------|
| `intent` | Yes | Human-readable purpose string |
| `scope` | Yes | Dotted path (e.g., `banking.transfers`) |
| `risk` | Yes | `low`, `medium`, `high`, or `critical` |
| `requires` | No | List of required capabilities |

### Use Declarations

Import modules or packages:

```
use embeddings
use openai as ai
```

Built-in modules (all 20) need no installation. File-based packages are loaded from `./covenant_packages/` or `~/.covenant/packages/`.

## Contracts

```
contract name(param1: Type1, param2: Type2) -> ReturnType
  precondition:
    condition1
    condition2

  postcondition:
    condition_about_result

  effects:
    modifies: [state1, state2]
    reads: [state3]
    emits: EventName
    touches_nothing_else

  permissions:
    grants: [perm1, perm2]
    denies: [perm3]
    escalation: policy_name

  body:
    -- implementation here
    return value

  on_failure:
    -- fallback if precondition/postcondition fails
    return default_value
```

### Sections

All sections except `body` are optional.

| Section | Purpose |
|---------|---------|
| `precondition` | Boolean expressions that must be true before body executes |
| `postcondition` | Boolean expressions that must be true after body executes. Can use `result` and `old()` |
| `effects` | Declared side effects: `modifies`, `reads`, `emits`, `touches_nothing_else` |
| `permissions` | Access control: `grants`, `denies`, `escalation` |
| `body` | The implementation |
| `on_failure` | Fallback logic if conditions fail |

### Parameters

Parameters have a name and type annotation:

```
contract fetch(url: String, timeout: Int) -> String
```

Supported type annotations:
- Simple: `Int`, `Float`, `String`, `Bool`, `List`, `Object`
- Annotated: `String [sensitive]`, `Int [positive]`
- Generic: `List<String>`, `Map<String, Int>`
- Custom: Any user-defined type name

### Preconditions

```
  precondition:
    amount > 0
    account.balance >= amount
    name != ""
```

Each line is a separate condition. All must be true.

### Postconditions

```
  postcondition:
    result > 0
    result == old(balance) - amount
    len(result) <= max_items
```

- `result` refers to the return value.
- `old(expr)` refers to the value of `expr` before the body executed.

### Effects

```
  effects:
    modifies: [account.balance, ledger]
    reads: [config.rate]
    emits: TransferCompleted
    touches_nothing_else
```

The Intent Verification Engine checks that your body matches these declarations.

## Types

| Type | Description | Examples |
|------|-------------|---------|
| `Int` | 64-bit signed integer | `42`, `-7`, `0` |
| `Float` | 64-bit floating point | `3.14`, `-0.5`, `1.0` |
| `String` | UTF-8 string | `"hello"`, `"line\n"` |
| `Bool` | Boolean | `true`, `false` |
| `List` | Ordered collection | `[1, 2, 3]`, `[]` |
| `Object` | Named record with fields | `Person(name: "Alice")` |
| `Null` | Absence of value | `null` |

### Objects (Constructors)

Any capitalized name acts as a constructor:

```
entry = Entry(text: "hello", score: 0.95)
point = Point(x: 10, y: 20)
```

Access fields with dot notation:

```
print(entry.text)
print(point.x)
```

### Custom Types

```
type Currency(String):
  fields:
    code: String [iso4217]
    amount: Float [positive]
  flow_constraints:
    never_flows_to: [logging, external_api]
```

### Shared State

```
shared request_count: Int:
  access: read-write
  isolation: exclusive
  audit: log_writes
```

## Operators

### Arithmetic

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Add, string concat, list concat | `3 + 4`, `"a" + "b"`, `[1] + [2]` |
| `-` | Subtract | `10 - 3` |
| `*` | Multiply | `4 * 5` |
| `/` | Divide (int if exact, float otherwise) | `10 / 3` gives `3.333...` |

### Comparison

| Operator | Description |
|----------|-------------|
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `<=` | Less than or equal |
| `>` | Greater than |
| `>=` | Greater than or equal |

### Logical

| Operator | Description |
|----------|-------------|
| `and` | Logical AND |
| `or` | Logical OR |
| `not` | Logical NOT |

### Index Access

```
items = ["a", "b", "c"]
first = items[0]           -- "a"
char = "hello"[1]          -- "e"
record = entries[i].name   -- chained with field access
```

### Assignment

```
x = 42
obj.field = "new value"    -- dotted path assignment
```

## Control Flow

### If / Else

```
if condition:
  -- then branch
else:
  -- else branch
```

Else is optional.

### While

```
while condition:
  -- loop body
```

### For

```
for item in collection:
  -- loop body

for i in range(10):
  print(i)
```

### Return

```
return value
```

### Emit

Emit named events:

```
emit TransferCompleted(from, to, amount)
emit UserLoggedIn(user_id)
```

## Built-in Functions

| Function | Description | Returns |
|----------|-------------|---------|
| `print(value, ...)` | Output to stdout | `null` |
| `len(list_or_string)` | Length | `Int` |
| `abs(number)` | Absolute value | `Int` or `Float` |
| `min(a, b)` | Minimum | same as input |
| `max(a, b)` | Maximum | same as input |
| `range(n)` | List `[0, 1, ..., n-1]` | `List` |
| `str(value)` | Convert to string | `String` |
| `int(value)` | Convert to integer | `Int` |
| `float(value)` | Convert to float | `Float` |

## Method Calls

Objects and strings support method calls:

```
-- List methods
items = [1, 2, 3]
more = items.append(4)       -- [1, 2, 3, 4]
n = items.len()              -- 3

-- String methods
s = "Hello World"
n = s.len()                  -- 11
up = s.upper()               -- "HELLO WORLD"
lo = s.lower()               -- "hello world"
has = s.contains("World")    -- true

-- Stdlib methods
answer = ollama.chat("hi")
hash = crypto.sha256("data")
```

## Keyword Arguments

Function and method calls support keyword arguments:

```
answer = ollama.chat("hello", model: "llama3.2", system: "Be helpful")
df = data.frame(columns: ["name", "age"], rows: [["Alice", 30]])
tmpl = prompts.template("Hi {name}", name: "Alice")
```

## Verification

### Intent Verification Engine (IVE)

Run `covenant check file.cov` to verify:

- **E001**: Body modifies state not declared in `effects`
- **E003**: `touches_nothing_else` violated — body calls undeclared functions
- **E004**: Missing body
- **E005**: Body emits events not declared in `effects`
- **W001**: Declared effect not observed in body
- **W003-W005**: Achievability/scope warnings (escalate at high/critical risk)
- **W006**: Declared `emits` not observed
- **W007**: `old()` references unmodified state
- **I001**: Recursion detected
- **I002**: Deep nesting detected

### Capability Type System (IFC)

- **F001**: Information flow violation
- **F002**: Permission denied
- **F003**: Missing required context
- **F004-F006**: Capability check failures

### Contract Verification

- **V001**: Missing return in body
- **V002**: Dead code after return
- **V003**: `on_failure` handler issues
- **V004-V005**: Shared state verification

### Behavioral Fingerprinting

```bash
covenant fingerprint file.cov
```

Shows reads, mutations, calls, events, old() references, branching, looping, recursion, and the intent hash for each contract.

## Execution Model

Covenant compiles to bytecode and runs on a stack-based VM by default:

```bash
covenant run file.cov -c main              # VM (default)
covenant run file.cov -c main --interpret   # Tree-walking interpreter
```

The VM is faster. The interpreter is useful for debugging.

### Arguments

Pass arguments as key=value:

```bash
covenant run file.cov -c factorial --arg n=10
```

### Pre-compilation

```bash
covenant build file.cov                     # Produces file.covc
covenant exec file.covc -c main             # Run bytecode directly
```
