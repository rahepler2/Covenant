# CLI Reference

## Commands

### `covenant run <file> [options]`

Execute a contract.

```bash
covenant run hello.cov -c main
covenant run math.cov -c factorial --arg n=10
covenant run demo.cov -c main --interpret    # use tree-walker instead of VM
```

| Flag | Description |
|------|-------------|
| `-c, --contract <name>` | Contract to execute (defaults to first) |
| `-a, --arg <key=value>` | Pass arguments (repeatable) |
| `--interpret` | Use tree-walking interpreter instead of bytecode VM |

Argument values are auto-detected: integers, floats, booleans (`true`/`false`), `null`, JSON objects, or strings.

### `covenant check <file>`

Run the Intent Verification Engine. Reports errors, warnings, and info.

```bash
covenant check transfer.cov
```

Output includes verification codes (S001-S003, E001-E005, W001-W008, I001-I002, F001-F006, V001-V005) and intent hashes.

### `covenant parse <file>`

Parse and display the AST structure.

```bash
covenant parse hello.cov
```

### `covenant tokenize <file>`

Display the token stream (for debugging).

```bash
covenant tokenize hello.cov
```

### `covenant fingerprint <file>`

Show behavioral fingerprints for all contracts: reads, mutations, calls, events, branching, looping, recursion, and intent hashes.

```bash
covenant fingerprint transfer.cov
```

### `covenant build <file> [-o output]`

Compile to bytecode (`.covc` file).

```bash
covenant build hello.cov                  # produces hello.covc
covenant build hello.cov -o out.covc      # custom output path
```

### `covenant exec <file> [options]`

Execute pre-compiled bytecode.

```bash
covenant exec hello.covc -c main
covenant exec hello.covc -c factorial --arg n=10
```

### `covenant disasm <file>`

Disassemble a `.cov` file — shows the bytecode instructions.

```bash
covenant disasm hello.cov
```

### `covenant init`

Initialize a new Covenant project in the current directory.

```bash
mkdir my-project && cd my-project
covenant init
```

Creates:
- `main.cov` — Starter contract
- `covenant_packages/` — Local package directory

### `covenant add <name> [--global]`

Install a package. Built-in modules are detected automatically.

```bash
covenant add mylib                  # local: ./covenant_packages/mylib/
covenant add mylib --global         # global: ~/.covenant/packages/mylib/
covenant add ollama                 # "ollama is built-in, no install needed"
```

### `covenant packages`

List all available modules and installed packages.

```bash
covenant packages
```

Output:
```
Built-in modules (always available):
  Tier 1: web, data, json, file, ai, crypto, time, math, text, env
  Tier 2: http, anthropic, openai, ollama, grok, mcp, mcpx, embeddings, prompts, guardrails

No file-based packages installed.
```

### `covenant map [dir] [options]`

Show the impact map of contracts, scopes, and dependencies across your project. Scans all `.cov` files recursively.

```bash
covenant map                                # full project map (scans current dir)
covenant map examples/                      # scan a specific directory
covenant map --contract transfer            # show impact of one contract
covenant map --file examples/transfer.cov   # show impact of one file
```

| Flag | Description |
|------|-------------|
| `-c, --contract <name>` | Show impact for a specific contract |
| `-f, --file <path>` | Show impact for all contracts in a specific file |

**Full map** shows:
- All scopes with their contracts, effects, and calls
- Cross-scope dependencies (which contracts call into other scopes)
- Shared state contention (which contracts write to the same state)

**Targeted view** (`--contract` or `--file`) shows:
- Direct effects (modifies, reads, emits)
- Level 1 calls with their effects
- Level 2 calls (what your calls call)
- Who calls this contract
- Shared state impact (other contracts reading/writing the same state)

Output is standard text — pipe to `grep` for filtering:

```bash
covenant map | grep "MODIFIES"              # find all mutations
covenant map | grep "finance"               # find finance-scoped contracts
```
