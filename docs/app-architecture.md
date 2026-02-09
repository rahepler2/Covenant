# Building Full Applications with Covenant

This guide explains how to build complete, production-style applications with Covenant — including database persistence, HTTP APIs, frontend integration, and project organization.

## The Big Picture

Covenant applications follow a **contracts-as-endpoints** architecture:

```
Frontend (HTML/JS/React/etc.)
        ↓ HTTP
covenant serve (auto-routes contracts to API endpoints)
        ↓
.cov files (contracts = verified business logic)
        ↓
db module (SQLite) / web module (external APIs) / AI modules
```

Every contract you write becomes an API endpoint. Preconditions become input validation. Postconditions become output guarantees. The `effects` block documents what each endpoint touches. This is what "AI-first" means in practice — your business logic is self-documenting, verified, and auditable by default.

## Quick Start

```bash
# Create a new project
covenant init

# Write your contracts (api.cov, etc.)
# Add a static/ directory for frontend files

# Start the server
covenant serve --static-dir static/ --port 8080

# Your contracts are now API endpoints:
#   GET  /api/list_items
#   POST /api/create_item
#   GET  /api/get_item?id=1
```

## Project Structure

```
my-app/
├── api.cov                # Main API contracts
├── auth.cov               # Authentication contracts (optional)
├── ai.cov                 # AI/LLM processing contracts (optional)
├── static/                # Frontend files
│   ├── index.html
│   ├── app.js
│   └── style.css
├── data/                  # Database files (auto-created)
│   └── app.db
└── covenant_packages/     # Local packages
```

For larger apps, split by domain:

```
my-app/
├── users.cov              # User management
├── posts.cov              # Post/content CRUD
├── search.cov             # Search and filtering
├── ai_features.cov        # AI-powered features
├── static/
│   ├── index.html
│   └── ...
└── data/
```

`covenant serve` scans all `.cov` files in the directory and maps every contract to an endpoint.

## Contracts as API Endpoints

The core insight: **a contract IS an endpoint**.

| Contract Feature | HTTP Equivalent |
|-----------------|-----------------|
| Contract name   | Route path (`/api/contract_name`) |
| Parameters      | Request body (POST) or query params (GET) |
| Preconditions   | Input validation (400 on failure) |
| Postconditions  | Output guarantees |
| Effects block   | Documents side effects |
| Return value    | JSON response body |
| on_failure      | Error fallback |

### Routing Convention

Routes are derived automatically from contract names:

- `get_*`, `list_*`, `show_*`, `find_*`, `search_*`, `count_*` → **GET** requests
- Contracts with no params and no effects → **GET** requests
- Everything else → **POST** requests

```
-- This becomes GET /api/list_users
contract list_users() -> List
  body:
    ...

-- This becomes POST /api/create_user
contract create_user(name: String, email: String) -> Object
  effects:
    modifies [database]
  body:
    ...
```

### Special Contracts

- **`init`** / **`setup`** — Run automatically when the server starts. Use for database initialization.
- **`main`** — Used by `covenant run`, skipped by `covenant serve`.

### Request/Response Format

**GET requests** pass parameters as query strings:
```
GET /api/get_user?id=42
```

**POST requests** pass parameters as JSON body:
```
POST /api/create_user
Content-Type: application/json

{"name": "Alice", "email": "alice@example.com"}
```

**All responses** return JSON:
```json
// Success
{"ok": true, "data": <contract return value>}

// Precondition failure (400)
{"ok": false, "error": "Precondition failed in 'create_user': name != \"\""}

// Runtime error (500)
{"ok": false, "error": "SQL error: ..."}
```

## Database

Covenant uses SQLite via the `db` module for persistence. No ORM, no migrations framework — just SQL in contracts with full verification.

### Basic Database Usage

```
contract init()
  effects:
    modifies [database]
  body:
    conn = db.open("data/app.db")
    db.execute(conn, "CREATE TABLE IF NOT EXISTS users (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      email TEXT UNIQUE NOT NULL,
      created_at TEXT DEFAULT CURRENT_TIMESTAMP
    )")
    return true
```

### CRUD Pattern

```
-- Create
contract create_user(name: String, email: String) -> Object
  precondition:
    name != ""
    email != ""
  effects:
    modifies [database]
  body:
    conn = db.open("data/app.db")
    db.execute(conn, "INSERT INTO users (name, email) VALUES (?, ?)", [name, email])
    rows = db.query(conn, "SELECT * FROM users ORDER BY id DESC LIMIT 1")
    return rows[0]

-- Read (single)
contract get_user(id: Int) -> Object
  precondition:
    id > 0
  body:
    conn = db.open("data/app.db")
    rows = db.query(conn, "SELECT * FROM users WHERE id = ?", [id])
    if rows == []
      return null
    return rows[0]

-- Read (list)
contract list_users() -> List
  body:
    conn = db.open("data/app.db")
    return db.query(conn, "SELECT * FROM users ORDER BY id DESC")

-- Update
contract update_user(id: Int, name: String) -> Bool
  precondition:
    id > 0
    name != ""
  effects:
    modifies [database]
  body:
    conn = db.open("data/app.db")
    db.execute(conn, "UPDATE users SET name = ? WHERE id = ?", [name, id])
    return true

-- Delete
contract delete_user(id: Int) -> Bool
  precondition:
    id > 0
  effects:
    modifies [database]
  body:
    conn = db.open("data/app.db")
    db.execute(conn, "DELETE FROM users WHERE id = ?", [id])
    return true
```

### db Module API

| Method | Description |
|--------|-------------|
| `db.open(path)` | Open/create a SQLite database, returns a Database object |
| `db.execute(conn, sql, [params])` | Execute a statement (INSERT, UPDATE, DELETE, CREATE) |
| `db.query(conn, sql, [params])` | Query rows, returns a List of Objects |
| `db.tables(conn)` | List all table names |
| `db.close(conn)` | Close connection (no-op, safe to call) |

Query results are returned as a List of Row objects. Each Row has fields matching the column names:
```
rows = db.query(conn, "SELECT id, name FROM users")
-- rows[0].id = 1
-- rows[0].name = "Alice"
```

Parameters use `?` placeholders to prevent SQL injection:
```
db.query(conn, "SELECT * FROM users WHERE name = ? AND age > ?", ["Alice", 25])
```

## Frontend Integration

Covenant serves static files alongside the API. Put your frontend in a `static/` directory.

### Vanilla HTML/JS

```html
<script>
  // Fetch data from a Covenant contract
  const res = await fetch('/api/list_users');
  const json = await res.json();

  if (json.ok) {
    const users = json.data;
    // render users...
  }

  // Create via POST
  await fetch('/api/create_user', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name: 'Alice', email: 'alice@example.com' })
  });
</script>
```

### With React / Vue / Any SPA

Covenant's serve command includes CORS headers, so you can develop your frontend separately:

```bash
# Terminal 1: Covenant API
covenant serve --port 8080

# Terminal 2: Frontend dev server (React, Vue, etc.)
npm run dev  # Usually port 3000 or 5173
```

In your frontend code, point API calls to `http://localhost:8080/api/...`.

For production, build your SPA and put the output in `static/`:
```bash
npm run build
cp -r dist/* static/
covenant serve --static-dir static/ --port 8080
```

## Adding AI Features

This is where Covenant shines. AI capabilities are built-in, not bolted on.

### Example: AI-Powered Search

```
intent: "AI-enhanced search over tasks"
scope: app.ai
risk: medium

requires:
  db
  ollama

contract smart_search(query: String) -> List
  precondition:
    query != ""
  body:
    conn = db.open("data/app.db")
    tasks = db.query(conn, "SELECT id, title, description FROM tasks")

    -- Use local LLM to rank relevance
    prompt = "Given the search query: '" + query + "'\n"
    prompt = prompt + "Rank these tasks by relevance (return IDs as comma-separated):\n"

    i = 0
    for task in tasks
      prompt = prompt + task.id + ": " + task.title + "\n"
      i = i + 1

    response = ollama.chat(prompt, model: "llama3.2")

    -- Filter tasks by AI ranking
    relevant = []
    for task in tasks
      id_str = "" + task.id
      if text.contains(response, id_str)
        relevant = relevant + [task]

    return relevant
```

### Example: Content Generation

```
contract generate_summary(task_id: Int) -> String
  precondition:
    task_id > 0
  body:
    conn = db.open("data/app.db")
    rows = db.query(conn, "SELECT * FROM tasks WHERE id = ?", [task_id])
    if rows == []
      return "Task not found"

    task = rows[0]
    summary = anthropic.chat(
      "Summarize this task in one sentence: " + task.title + " - " + task.description,
      model: "claude-sonnet-4-20250514"
    )
    return summary
```

### Example: Guardrails on User Input

```
contract create_post(title: String, content: String) -> Object
  precondition:
    title != ""
    content != ""
  body:
    -- Validate content before saving
    has_pii = guardrails.check_pii(content)
    if has_pii != false
      return Error(message: "Content contains personal information: " + has_pii)

    bad_words = guardrails.check_not_contains(content, words: ["spam", "scam"])
    if bad_words == false
      return Error(message: "Content contains prohibited words")

    conn = db.open("data/app.db")
    db.execute(conn, "INSERT INTO posts (title, content) VALUES (?, ?)", [title, content])
    return Success(saved: true)
```

## Architecture Patterns

### Pattern 1: Simple CRUD App

The most common pattern. One `.cov` file per resource, SQLite for storage, vanilla frontend.

```
my-app/
├── tasks.cov       # Task CRUD
├── users.cov       # User CRUD
└── static/
    └── index.html
```

```bash
covenant serve --static-dir static/
```

### Pattern 2: AI Backend

Covenant as a backend for AI processing. Frontend can be anything — React, mobile app, another service.

```
ai-service/
├── embeddings.cov    # Vector operations
├── chat.cov          # LLM interactions
├── rag.cov           # Retrieval-augmented generation
└── data/
    └── vectors.db    # Stored embeddings
```

```bash
covenant serve --port 3000
# Frontend calls http://localhost:3000/api/chat, /api/search, etc.
```

### Pattern 3: Data Pipeline

No frontend. Contracts process data files and write results.

```
pipeline/
├── ingest.cov       # Read CSV/JSON inputs
├── transform.cov    # Clean and transform
├── analyze.cov      # Statistical analysis
└── report.cov       # Generate output
```

```bash
covenant run ingest.cov -c process_all
covenant run analyze.cov -c generate_report
```

### Pattern 4: MCP Integration

Covenant orchestrating multiple MCP servers for complex AI workflows.

```
orchestrator/
├── tools.cov         # MCP server connections
├── workflow.cov      # Multi-step AI pipelines
└── api.cov           # HTTP endpoints
```

## Running in Production

### Build and Deploy

```bash
# Verify all contracts
covenant check *.cov

# Build optimized bytecode (optional — faster startup)
covenant build api.cov -o api.covc

# Start server
covenant serve --host 0.0.0.0 --port 8080 --static-dir static/
```

### Behind a Reverse Proxy (nginx)

```nginx
server {
    listen 80;
    server_name myapp.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
    }
}
```

### Environment Variables

Use the `env` module for configuration:

```
contract init()
  body:
    port = env.get("PORT", "8080")
    db_path = env.get("DATABASE_PATH", "data/app.db")
    conn = db.open(db_path)
    ...
```

## Full Example: Task Manager

See `examples/fullstack/` for a complete working example:

```bash
cd examples/fullstack
covenant serve --static-dir static/ --port 8080
# Open http://localhost:8080
```

This includes:
- `api.cov` — Full CRUD API with SQLite
- `static/index.html` — Single-page frontend
- Auto-initialization of database on server start
- Input validation via preconditions
- Every operation is a verified contract

## What Covenant Gives You (vs. Express/Flask/Django)

| Feature | Traditional | Covenant |
|---------|-------------|----------|
| Input validation | Manual (express-validator, etc.) | Automatic (preconditions) |
| Output guarantees | None | Automatic (postconditions) |
| Side effect tracking | None | Declared (effects block) |
| API documentation | Swagger/OpenAPI (separate) | Built into the code (intent, scope) |
| Security audit | Manual review | `covenant check` (automated) |
| Risk assessment | Manual | Declared per-file, auto-escalated |
| AI integration | Import libraries, configure | Built-in (5 LLM providers + embeddings) |
| Type safety | TypeScript/mypy (separate) | Gradual types (built-in) |
| Behavioral fingerprinting | None | `covenant fingerprint` |

## Summary

The Covenant development flow:

1. **`covenant init`** — Create project
2. **Write `.cov` files** — Each contract = one endpoint
3. **`covenant check *.cov`** — Verify intent, effects, types
4. **`covenant serve`** — Start server, contracts become API
5. **Build frontend** — Call `/api/<contract_name>` from JS
6. **`covenant map`** — Understand contract dependencies

Contracts are the unit of everything: business logic, API routing, verification, and documentation — all in one place.
