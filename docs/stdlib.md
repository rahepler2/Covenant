# Standard Library Reference

Covenant ships with 20 built-in modules. Call any method with `module.method()` — no imports needed, though `use module` is recommended for clarity.

## Tier 1 — Core Modules

### math

Mathematical functions and constants.

```
root = math.sqrt(144)        -- 12.0
val = math.pow(2, 10)        -- 1024
f = math.floor(3.7)          -- 3
c = math.ceil(3.2)           -- 4
r = math.round(3.5)          -- 4
s = math.sin(0.0)            -- 0.0
co = math.cos(0.0)           -- 1.0
t = math.tan(0.0)            -- 0.0
l = math.log(1.0)            -- 0.0 (natural log)
l10 = math.log10(100.0)      -- 2.0
e = math.exp(1.0)            -- 2.718...
pi = math.pi                 -- 3.14159...
euler = math.e               -- 2.71828...
rnd = math.random()          -- 0.0 to 1.0
```

### text

String manipulation.

```
words = text.split("a b c")                -- ["a", "b", "c"]
words = text.split("a,b,c", ",")           -- ["a", "b", "c"]
joined = text.join("-", ["a", "b", "c"])    -- "a-b-c"
up = text.upper("hello")                    -- "HELLO"
lo = text.lower("HELLO")                    -- "hello"
t = text.trim("  hi  ")                     -- "hi"
r = text.replace("hello world", "world", "there")  -- "hello there"
s = text.starts_with("hello", "he")         -- true
e = text.ends_with("hello", "lo")           -- true
c = text.contains("hello", "ell")           -- true
m = text.matches("test@example.com", "[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}") -- true
all = text.find_all("a1b2c3", "[0-9]+")    -- ["1", "2", "3"]
rep = text.repeat("ab", 3)                  -- "ababab"
rev = text.reverse("hello")                 -- "olleh"
n = text.length("hello")                    -- 5
sub = text.slice("hello", 1, 3)             -- "el"
```

### json

JSON parsing and serialization.

```
obj = json.parse("{\"name\": \"Alice\", \"age\": 30}")
print(obj.name)              -- "Alice"

str = json.stringify(obj)    -- {"age":30,"name":"Alice"}
```

### file

File system operations (max 10MB reads).

```
content = file.read("/tmp/data.txt")
ok = file.write("/tmp/out.txt", "hello")
ok = file.append("/tmp/out.txt", "\nmore")
exists = file.exists("/tmp/out.txt")         -- true
lines = file.lines("/tmp/data.txt")          -- ["line1", "line2", ...]
ok = file.delete("/tmp/out.txt")
```

### crypto

Cryptographic hashing.

```
hash = crypto.sha256("Hello, World!")
-- "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"

mac = crypto.hmac("secret-key", "message")
-- HMAC-SHA256 hex string
```

### time

Time operations.

```
now = time.now()                -- 1707412345.123 (float, with fractional seconds)
ts = time.timestamp()           -- 1707412345 (integer)
fmt = time.format(ts)           -- "2024-02-08 19:32:25"
elapsed = time.elapsed(start)   -- seconds since start
time.sleep(100)                 -- sleep 100ms (max 60000)
```

### env

Environment variables.

```
home = env.get("HOME")
val = env.get("MISSING", "default")     -- returns "default" if not set
env.set("MY_VAR", "value")
has = env.has("HOME")                    -- true
all = env.all()                          -- object with all vars
```

### data

DataFrame operations for tabular data.

```
df = data.frame(
  columns: ["name", "age", "score"],
  rows: [
    ["Alice", 30, 95],
    ["Bob", 25, 87],
    ["Carol", 35, 92]
  ]
)

df.print()                              -- formatted table
count = df.count()                      -- 3
avg = df.mean("age")                    -- 30.0
total = df.sum("score")                 -- 274
names = df.column("name")              -- ["Alice", "Bob", "Carol"]

-- Filtering and sorting
young = df.filter("age", "<", 30)
sorted = df.sort_by("score", "desc")
top = df.head(2)

-- CSV
csv_str = df.to_csv()
df2 = data.read_csv("/tmp/data.csv")
```

### web

HTTP client.

```
resp = web.get("https://api.example.com/data")
print(resp.status)                      -- 200
print(resp.ok)                          -- true
data = resp.json()                      -- parsed JSON
body = resp.text()                      -- raw body string

resp = web.post(
  "https://api.example.com/submit",
  body: Request(name: "test"),
  headers: Headers(Authorization: "Bearer token123")
)
```

Methods: `get`, `post`, `put`, `delete`. Supports kwargs: `headers`, `auth`, `timeout`.

### ai

High-level AI operations (uses Anthropic or OpenAI API key from env).

```
answer = ai.prompt("What is Rust?")
summary = ai.summarize("Long text here...")
category = ai.classify("I love this!", categories: ["positive", "negative"])
data = ai.extract("Alice is 30 years old", fields: ["name", "age"])
```

---

## Tier 2 — AI-Age Modules

### http

Full HTTP client (like Python's `requests`).

```
resp = http.get("https://api.example.com/data", timeout: 30)
resp = http.post("https://api.example.com", json: Payload(key: "value"))
resp = http.put(url, body: "raw data")
resp = http.patch(url, json: Update(field: "new"))
resp = http.delete(url, auth: "bearer-token")
resp = http.head(url)
```

All methods return `HttpResponse` with `.json()`, `.text()`, `.status`, `.ok`.

### ollama

Local LLM via [Ollama](https://ollama.com).

```
-- Chat (default model: llama3.2)
answer = ollama.chat("What is Covenant?")
answer = ollama.chat("Explain this code", model: "codellama")
answer = ollama.chat("Help me", system: "You are a pirate")

-- Completions
text = ollama.generate("Once upon a time")

-- Embeddings (default model: nomic-embed-text)
vec = ollama.embed("some text to embed")

-- Model management
models = ollama.list()
ollama.pull("llama3.2")

-- Custom server
answer = ollama.chat("hi", url: "http://my-server:11434")
```

### anthropic

Anthropic Claude API. Requires `ANTHROPIC_API_KEY` env var.

```
answer = anthropic.chat("What is quantum computing?")
answer = anthropic.chat(
  "Explain monads",
  model: "claude-sonnet-4-20250514",
  max_tokens: 2048,
  temperature: 0.7,
  system: "You are a Haskell expert"
)
models = anthropic.models()
```

### openai

OpenAI API. Requires `OPENAI_API_KEY` env var.

```
answer = openai.chat("Write a haiku")
answer = openai.chat("Explain REST", model: "gpt-4o", temperature: 0.5)

-- Embeddings
vec = openai.embed("text to embed")
vecs = openai.embed(["text 1", "text 2"])

-- Images
url = openai.image("A cat in space", size: "1024x1024")

-- Compatible endpoints (e.g., local vLLM)
answer = openai.chat("hi", base_url: "http://localhost:8000/v1")

models = openai.models()
```

### grok

xAI Grok API. Requires `XAI_API_KEY` env var.

```
answer = grok.chat("What happened today?")
answer = grok.chat("Explain AI", model: "grok-2-latest", temperature: 0.8)
models = grok.models()
```

### mcp

[Model Context Protocol](https://modelcontextprotocol.io) client. Connects to MCP servers via stdio or HTTP.

```
-- Connect to a server
server = mcp.connect("npx @modelcontextprotocol/server-filesystem /tmp")

-- List and call tools
tools = mcp.list_tools(server)
result = mcp.call_tool(server, "read_file", path: "/tmp/data.txt")

-- Resources
resources = mcp.list_resources(server)
content = mcp.get_resource(server, "file:///tmp/data.txt")

-- Prompt templates
response = mcp.prompt(server, "summarize", text: "long content here")

-- HTTP transport
server = mcp.connect("http://localhost:3000/mcp", transport: "http")
```

### mcpx

MCP extensions for multi-server coordination.

```
-- Route requests across servers
router = mcpx.router(servers: [
  Server(name: "fs", server: fs_conn, tools: ["read_file", "write_file"]),
  Server(name: "db", server: db_conn, tools: ["query", "insert"])
])

-- Sequential pipeline
result = mcpx.chain(steps: [
  Step(server: conn1, tool: "fetch_data", args: Args(url: "...")),
  Step(server: conn2, tool: "process", args: Args(format: "json"))
])

-- Parallel execution
results = mcpx.parallel(tasks: [
  Task(server: conn, tool: "analyze", args: Args(text: doc1)),
  Task(server: conn, tool: "analyze", args: Args(text: doc2))
])

-- Fallback (try until one succeeds)
result = mcpx.fallback(alternatives: [
  Alt(server: primary, tool: "search", args: Args(q: "test")),
  Alt(server: backup, tool: "search", args: Args(q: "test"))
])
```

### embeddings

Vector math for semantic search and similarity.

```
-- Similarity
score = embeddings.cosine(vec1, vec2)           -- -1.0 to 1.0
dp = embeddings.dot(vec1, vec2)                  -- dot product

-- Search
result = embeddings.nearest(query_vec, candidates, k: 3)
-- Returns list of SearchResult(index, score, item)

-- Vector operations
normed = embeddings.normalize(vector)            -- unit vector
dist = embeddings.distance(vec1, vec2)           -- euclidean distance
mag = embeddings.magnitude(vector)               -- vector length
sum = embeddings.add(vec1, vec2)
diff = embeddings.sub(vec1, vec2)
scaled = embeddings.scale(vector, 2.0)
dims = embeddings.dim(vector)                    -- dimension count
```

### prompts

Prompt engineering toolkit.

```
-- Template substitution
msg = prompts.template(
  "Hello {name}, you have {count} messages",
  name: "Alice",
  count: "5"
)

-- Few-shot prompting
prompt = prompts.few_shot(
  task: "Classify sentiment",
  examples: [
    Example(input: "I love this!", output: "positive"),
    Example(input: "This is terrible", output: "negative")
  ],
  input: "Pretty good actually"
)

-- Message builders (for chat APIs)
sys = prompts.system("You are a helpful assistant")
usr = prompts.user("What is 2+2?")
ast = prompts.assistant("4")

-- Build message list
msgs = prompts.messages(
  system: "You are helpful",
  history: [usr, ast],
  user: "What about 3+3?"
)

-- Structured prompt
prompt = prompts.format(
  context: "User is asking about Python",
  instructions: "Provide a code example",
  constraints: "Keep it under 10 lines",
  output_format: "markdown code block"
)
```

### guardrails

Output validation and safety checks.

```
-- JSON validation
is_valid = guardrails.validate_json("{\"key\": \"value\"}")    -- true
is_valid = guardrails.validate_json("not json")                 -- false

-- Schema validation
ok = guardrails.validate_schema(
  data,
  required: ["name", "age"],
  types: TypeMap(name: "string", age: "number")
)

-- PII detection
result = guardrails.check_pii("Contact alice@example.com")
-- Returns ["email"] (list of findings)
result = guardrails.check_pii("The weather is nice")
-- Returns false (no PII)
-- Detects: email, phone, ssn, credit_card

-- Length checking
ok = guardrails.check_length(text, min: 10, max: 1000)

-- Sanitization
clean = guardrails.sanitize(text, strip_html: true, strip_code: true)

-- Format assertion
ok = guardrails.assert_format("test@example.com", "email")
-- Formats: json, email, url, number, integer, boolean, nonempty

-- Content filtering
ok = guardrails.check_contains(text, all: ["required", "words"])
ok = guardrails.check_not_contains(text, words: ["banned", "words"])

-- Extract JSON from LLM output (handles markdown, code blocks)
parsed = guardrails.retry_parse("Here is the JSON: ```json{\"key\": 1}```")
```
