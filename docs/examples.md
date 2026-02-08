# Examples & Patterns

## Hello World

```
intent: "Hello world"
scope: demo
risk: low

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    print("Hello, World!")
    return 0
```

## Factorial (Recursion)

```
intent: "Calculate factorial"
scope: math
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

## FizzBuzz

```
intent: "FizzBuzz"
scope: demo
risk: low

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    for i in range(100):
      n = i + 1
      if n % 15 == 0:
        print("FizzBuzz")
      else:
        if n % 3 == 0:
          print("Fizz")
        else:
          if n % 5 == 0:
            print("Buzz")
          else:
            print(n)
    return 0
```

## Working with Lists

```
intent: "List operations"
scope: demo
risk: low

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    -- Create and index
    fruits = ["apple", "banana", "cherry", "date"]
    print(fruits[0])
    print(fruits[3])

    -- Iterate
    for fruit in fruits:
      print(text.upper(fruit))

    -- Build a list
    squares = []
    for i in range(10):
      squares = squares + [i * i]
    print(squares)

    -- Length
    print("Count: " + str(len(fruits)))

    return 0
```

## Working with Objects

```
intent: "Object patterns"
scope: demo
risk: low

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    -- Create objects with keyword args
    alice = Person(name: "Alice", age: 30, role: "engineer")
    bob = Person(name: "Bob", age: 25, role: "designer")

    people = [alice, bob]
    for p in people:
      print(p.name + " is " + str(p.age))

    return 0
```

## Ask an LLM (Ollama)

```
intent: "Chat with a local LLM"
scope: demo.ai
risk: low

use ollama

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    answer = ollama.chat(
      "What are the three laws of robotics?",
      system: "Answer concisely in bullet points"
    )
    print(answer)
    return 0
```

## Ask Claude (Anthropic)

Requires `ANTHROPIC_API_KEY` environment variable.

```
intent: "Chat with Claude"
scope: demo.ai
risk: low

use anthropic

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    answer = anthropic.chat(
      "Explain the difference between concurrency and parallelism",
      model: "claude-sonnet-4-20250514",
      max_tokens: 500
    )
    print(answer)
    return 0
```

## RAG (Retrieval-Augmented Generation)

Full RAG pipeline: embed documents, search by similarity, augment prompt, ask LLM, validate output.

```
intent: "Answer questions from a knowledge base using RAG"
scope: demo.rag
risk: low

use ollama
use embeddings
use prompts
use guardrails

contract build_index(chunks: List) -> List
  precondition:
    len(chunks) > 0
  postcondition:
    len(result) == len(chunks)
  body:
    index = []
    i = 0
    while i < len(chunks):
      vec = ollama.embed(chunks[i])
      index = index + [Entry(text: chunks[i], vector: vec)]
      i = i + 1
    return index

contract search(query: String, index: List, k: Int) -> List
  precondition:
    len(index) > 0
  postcondition:
    len(result) <= k
  body:
    query_vec = ollama.embed(query)
    vectors = []
    i = 0
    while i < len(index):
      vectors = vectors + [index[i].vector]
      i = i + 1
    results = embeddings.nearest(query_vec, vectors, k: k)
    matched = []
    i = 0
    while i < len(results):
      matched = matched + [index[results[i].index].text]
      i = i + 1
    return matched

contract ask(question: String, index: List) -> String
  precondition:
    question != ""
  postcondition:
    result != ""
  body:
    chunks = search(question, index, 3)
    context = text.join("\n\n", chunks)
    augmented = prompts.template(
      "Answer based ONLY on this context:\n\n{context}\n\nQuestion: {question}",
      context: context,
      question: question
    )
    answer = ollama.chat(augmented, system: "Be concise and accurate.")
    has_pii = guardrails.check_pii(answer)
    if has_pii != false:
      return "Response blocked: contains personal information."
    return answer

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    chunks = [
      "Covenant uses contracts instead of functions.",
      "Every contract has a precondition, postcondition, and body.",
      "The standard library has 20 built-in modules."
    ]
    index = build_index(chunks)
    answer = ask("What is a contract?", index)
    print(answer)
    return 0
```

## Prompt Engineering

```
intent: "Prompt engineering patterns"
scope: demo.prompts
risk: low

use prompts

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    -- Template substitution
    msg = prompts.template(
      "Dear {name},\n\nYour order #{order} has shipped.\n\nThanks,\n{company}",
      name: "Alice",
      order: "12345",
      company: "Acme Corp"
    )
    print(msg)
    print("")

    -- Build chat messages
    sys = prompts.system("You are a helpful coding assistant")
    usr = prompts.user("How do I reverse a string?")
    msgs = prompts.messages(system: "Be concise", user: "What is 2+2?")
    print(msgs)

    return 0
```

## Data Processing

```
intent: "Process tabular data"
scope: demo.data
risk: low

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    df = data.frame(
      columns: ["name", "dept", "salary"],
      rows: [
        ["Alice", "Engineering", 120000],
        ["Bob", "Design", 95000],
        ["Carol", "Engineering", 130000],
        ["Dave", "Marketing", 85000],
        ["Eve", "Engineering", 115000]
      ]
    )

    print("All employees:")
    df.print()
    print("")

    eng = df.filter("dept", "==", "Engineering")
    print("Engineering team:")
    eng.print()
    print("")

    avg = eng.mean("salary")
    print("Average engineering salary:")
    print(avg)

    sorted = df.sort_by("salary", "desc")
    print("")
    print("Top earners:")
    sorted.head(3).print()

    return 0
```

## HTTP API Client

```
intent: "Fetch data from a REST API"
scope: demo.http
risk: low

use http
use json

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    resp = http.get("https://httpbin.org/json")
    if resp.ok:
      data = resp.json()
      print(data)
    else:
      print("Request failed with status: " + str(resp.status))
    return 0
```

## File Processing

```
intent: "Read, process, and write files"
scope: demo.files
risk: low

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    -- Write a file
    file.write("/tmp/names.txt", "Alice\nBob\nCarol\nDave")

    -- Read lines
    lines = file.lines("/tmp/names.txt")
    print("Names:")
    for name in lines:
      print("  - " + text.upper(name))

    -- Write processed output
    upper_names = []
    for name in lines:
      upper_names = upper_names + [text.upper(name)]
    output = text.join("\n", upper_names)
    file.write("/tmp/upper_names.txt", output)

    print("Wrote " + str(len(upper_names)) + " names")

    -- Cleanup
    file.delete("/tmp/names.txt")
    file.delete("/tmp/upper_names.txt")
    return 0
```

## Verified Transfer (Design by Contract)

```
intent: "Transfer funds between accounts with full verification"
scope: banking.transfers
risk: high

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

  on_failure:
    emit TransferFailed(from, to, amount)
    return false
```

Run `covenant check transfer.cov` to verify the intent matches the implementation.

## Vector Similarity Search

```
intent: "Semantic search with embeddings"
scope: demo.search
risk: low

use embeddings

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    -- Sample vectors (in production, use ollama.embed())
    docs = [
      Doc(text: "Python is great for data science", vec: [0.9, 0.1, 0.3]),
      Doc(text: "Rust is fast and safe", vec: [0.1, 0.9, 0.2]),
      Doc(text: "JavaScript runs in the browser", vec: [0.2, 0.3, 0.9]),
      Doc(text: "Python has many ML libraries", vec: [0.85, 0.15, 0.25])
    ]

    query = [0.8, 0.2, 0.3]
    vectors = []
    i = 0
    while i < len(docs):
      vectors = vectors + [docs[i].vec]
      i = i + 1

    results = embeddings.nearest(query, vectors, k: 2)
    print("Most similar to query:")
    for hit in results:
      print("  " + docs[hit.index].text + " (score: " + str(hit.score) + ")")

    return 0
```

## Output Validation

```
intent: "Validate LLM output before using it"
scope: demo.guardrails
risk: low

use guardrails

contract main() -> Int
  precondition:
    true
  postcondition:
    result == 0
  body:
    -- Simulate an LLM response
    response = "Here is the data: {\"name\": \"Alice\", \"score\": 95}"

    -- Extract JSON from the response
    parsed = guardrails.retry_parse(response)
    print(parsed)

    -- Check for PII before logging
    text_to_log = "Contact support at help@example.com or 555-123-4567"
    pii = guardrails.check_pii(text_to_log)
    if pii != false:
      print("WARNING: Found PII types:")
      print(pii)
      print("Sanitizing before logging...")
    else:
      print("No PII detected, safe to log")

    -- Validate format
    print(guardrails.assert_format("user@test.com", "email"))
    print(guardrails.assert_format("not-an-email", "email"))

    return 0
```
