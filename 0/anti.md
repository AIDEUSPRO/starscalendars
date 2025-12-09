# ❌ Rust Production Anti-Patterns — Zero Tolerance (2025)

> **Purpose**: Hard rules for production-grade high-load systems based on official Rust stdlib documentation  
> **Sources**: https://doc.rust-lang.org/stable/std/ + performance research + production analysis  
> **Updated**: January 2025 based on Rust 1.90+ stable documentation  
> **Enforcement**: clippy/scripts/CI + mandatory agent compliance  

## 1) ERROR HANDLING: Zero-Tolerance Production Rules

### 🚫 ABSOLUTELY FORBIDDEN

Based on official Result enum documentation:

- **`unwrap()`, `expect()`, `panic!()`** — "Generally discouraged" per stdlib docs, kills availability
- **ALL `unwrap_or*` family** — includes `unwrap_or(...)`, `unwrap_or_default()`, and `unwrap_or_else(...)`
- **Custom unwrap variants** — `unwrap_u64`, `unwrap_str`, etc. - explicit type checking required
- **Error suppression** — Ignoring `Result`/`Option` without explicit handling violates Rust principles
- **Panic in libraries** — "Panics are meant for unrecoverable errors" - forces termination on users

### ✅ MANDATORY PATTERNS (Per Official Documentation)

**Use `?` operator for elegant error propagation:**
```rust
// ✅ Recommended by stdlib docs
let value = operation()?;
```

**Pattern matching for explicit error handling:**
```rust
// ✅ Official recommendation
match operation() {
    Ok(value) => process(value),
    Err(e) => handle_error(e),
}
```

**Lazy evaluation without unwraps:**
```rust
// ✅ Use match/if-let for identity right arm (preferred style)
let value = match operation() {
    Ok(v) => v,
    Err(_) => fallback(),
};

// ✅ Use map_or/map_or_else for non-identity transforms
let x = maybe_value.map_or(default(), |v| transform(v));
let y = result.map_or_else(|_| compute_default(), |v| transform(v));
```

### 🔁 Replacement Guidance (Strict Ban Compliance)

The following replacements are REQUIRED throughout the codebase:

- unwrap():
```rust
// ❌ value.unwrap()
// ✅
let value = match value_result { Ok(v) => v, Err(e) => return Err(e) };
// or simply propagate
let value = value_result?;
```

- expect("msg"):
```rust
// ❌ value.expect("msg")
// ✅
let value = match value_result { Ok(v) => v, Err(e) => return Err(e) };
```

- panic!():
```rust
// ❌ panic!("...")
// ✅ return Err(DomainError::InvalidInput(...)) // or appropriate error type
```

- unwrap_or(...):
```rust
// ❌ res.unwrap_or(fallback)
// ✅ identity right arm → match/if let
let v = match res { Ok(v) => v, Err(_) => fallback };
// ✅ Option
let v = opt.map_or(fallback, |v| v);
```

- unwrap_or_default():
```rust
// ❌ opt.unwrap_or_default()
// ✅ Option
let v = opt.map_or_else(Default::default, |v| v);
// ✅ Result
let v = match res { Ok(v) => v, Err(_) => Default::default() };
```

- unwrap_or_else(...):
```rust
// ❌ res.unwrap_or_else(|_| fallback())
// ✅ Result (identity right arm)
let v = match res { Ok(v) => v, Err(_) => fallback() };
// ✅ Option transform
let v = opt.map_or_else(|| fallback(), |v| transform(v));
```

- println!/eprintln!/dbg!:
```rust
// ❌ println!("..."), eprintln!("..."), dbg!(x)
// ✅ structured logs
tracing::info!(event = "...", key = %value, "message");
tracing::error!(error = ?err, "failure");
```

Notes:
- For Result/Option cases where the success branch is identity, prefer `match`/`if let`/`let-else` to avoid clippy suggestions towards unwrap_*.
- Use `map_or`/`map_or_else` only when performing a transformation in the success branch. For pure identity, avoid them.

**Result iterators for functional error handling:**
```rust
// ✅ Zero-copy iteration over Result values
result.iter().map(|value| transform(value)).collect()
result.into_iter().for_each(|value| process(value));
```

### Error Architecture Requirements

Use structured error types with proper propagation chains:

```rust
#[derive(thiserror::Error, Debug)]
pub enum DomainError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Business rule violation: {0}")]
    BusinessRule(String),
}
```

## 2) MEMORY ALLOCATION: Zero-Cost Production Rules

### 🚫 ALLOCATION ANTI-PATTERNS

Based on stdlib collection documentation:

- **Unplanned growth** — `Vec::new()` when size is known causes O(n) reallocations
- **Default allocation** — `HashMap::new()`, `String::new()` without capacity planning
- **Repeated reallocations** — Growing collections in loops without pre-allocation
- **Memory waste** — Not using `shrink_to_fit()` after bulk operations
- **Clone overuse** — Unnecessary deep copies when references suffice

### ✅ ZERO-COST ALLOCATION PATTERNS

**Pre-allocation for known sizes:**
```rust
// ✅ Single O(1) allocation
let mut items = Vec::with_capacity(known_size);
let mut map = HashMap::with_capacity(expected_entries);  
let mut buffer = String::with_capacity(estimated_length);
```

**Zero-copy operations with iterators:**
```rust
// ✅ No allocations - iterator chains
items.iter().filter(|x| condition(x)).map(|x| transform(x))
```

## 3) ASYNC PERFORMANCE: Cooperative Scheduling Rules

### 🚫 TOKIO RUNTIME KILLERS

Based on tokio documentation and production analysis:

- **`.await` in tight loops** — Starves cooperative scheduler, destroys p95 latency
- **Blocking I/O in async** — `std::fs`, `std::thread::sleep`, DNS lookups kill runtime
- **Long-running computations** — >1ms CPU work without yielding blocks other tasks
- **Unbounded task spawning** — Memory exhaustion through uncontrolled concurrency
- **Missing timeouts** — Hanging connections exhaust system resources

### ✅ COOPERATIVE ASYNC PATTERNS

**Bounded concurrent processing:**
```rust
// ✅ Limits concurrent operations
stream::iter(items)
    .map(|item| process_item(item))
    .buffered(50)  // Limit to 50 concurrent operations
    .collect().await
```

**Delegated blocking operations:**
```rust
// ✅ Isolate blocking I/O
tokio::task::spawn_blocking(move || {
    std::fs::read_to_string(path)
}).await??
```

## 4) SQL/DATABASE: Compile-Time Safety

### 🚫 SQL INJECTION & PERFORMANCE KILLERS

- **String concatenation** — `format!()` in SQL creates injection vulnerabilities
- **Dynamic queries** — Runtime SQL composition without validation
- **N+1 patterns** — Queries in loops instead of JOINs
- **Unbounded results** — Missing LIMIT clauses cause OOM
- **No transactions** — Multi-step writes without atomicity

### ✅ COMPILE-TIME CHECKED SQL

**Use sqlx macros for type safety:**
```rust
// ✅ Compile-time validation
sqlx::query_as!(User, "SELECT id, email FROM users WHERE id = $1", user_id)
```

**Atomic transactions:**
```rust
// ✅ Both operations or neither
let mut tx = pool.begin().await?;
// ... operations
tx.commit().await?;
```

## 5) CONCURRENCY: Lock-Free Patterns

### 🚫 CONCURRENCY KILLERS

- **`Arc<Mutex<T>>` overuse** — Contention destroys performance on hot paths
- **Blocking across `.await`** — Holding locks during async operations causes deadlocks
- **Global mutable state** — Shared mutation without proper synchronization
- **Unbounded channels** — Memory leaks through channel saturation

### ✅ LOCK-FREE PATTERNS

**Atomic operations for metrics:**
```rust
// ✅ Lock-free counters
use std::sync::atomic::{AtomicU64, Ordering};
counter.fetch_add(1, Ordering::Relaxed);
```

**Lock-free data structures:**
```rust
// ✅ Concurrent HashMap without locks
use dashmap::DashMap;
let cache: DashMap<String, Value> = DashMap::new();
```

## 6) HTTP/API: Type-Safe Zero-Copy

### 🚫 HTTP PERFORMANCE KILLERS

- **Runtime parsing** — String-based extractors with allocation overhead
- **Missing size limits** — Unbounded request bodies cause OOM attacks  
- **No rate limiting** — Resource exhaustion from abuse
- **Untyped responses** — Runtime serialization errors

### ✅ TYPE-SAFE HTTP PATTERNS

**Typed extractors:**
```rust
// ✅ Compile-time validation
#[derive(Deserialize, Validate)]
struct Request {
    #[validate(email)]
    email: String,
}
```

**Zero-copy path extraction:**
```rust
// ✅ Direct parsing, no allocation
Path(user_id): Path<Uuid>
```

## 7) OBSERVABILITY: Zero-PII Logging

### 🚫 LOGGING ANTI-PATTERNS

- **PII exposure** — Passwords, tokens, personal data in logs
- **Unstructured logs** — String concatenation without correlation
- **Missing trace IDs** — No request correlation
- **Blocking log I/O** — Synchronous writes on critical path

### ✅ STRUCTURED LOGGING

**PII-safe structured logs:**
```rust
// ✅ No sensitive data, structured fields
tracing::info!(
    user_id = %user_id,
    operation = "create_user", 
    duration_ms = duration.as_millis(),
    "User operation completed"
);
```

## 8) AGENT COMPLIANCE: Mandatory Rules

### All Specialized Agents MUST:

1. **Validate anti.md** before ANY code generation
2. **Reference official Rust docs** for stdlib usage patterns
3. **Explain violations** with specific anti.md section references
4. **Provide compliant alternatives** using documented best practices
5. **Fail fast** on anti-pattern detection with clear explanations

### Agent Code Generation Rules:

- **NO code examples in agents** - avoid obsolescence, focus on principles
- **Reference patterns** - point to anti.md rules and official documentation
- **Validate compliance** - check every suggestion against zero-tolerance rules
- **Explain reasoning** - cite official Rust documentation sources

## 9) ENFORCEMENT: Quality Gates

### CI/CD Pipeline Requirements:

```bash
# Mandatory checks (must pass)
cargo clippy -- -D warnings           # Zero tolerance for warnings
rg -n "\.unwrap\(\)" --type rust .    # Fail on unwrap usage
rg -n "\.expect\(" --type rust .      # Fail on expect usage  
rg -n "panic!\(" --type rust .        # Fail on panic usage
```

### Quality Gate Integration:

- **make quality-check** - Must pass before commits
- **Anti-pattern detection** - Automated scanning in CI
- **Agent validation** - All agent outputs must comply
- **Documentation compliance** - Agents must reference official sources

---

## References (Official Documentation)

### Primary Sources:
- [Rust Standard Library](https://doc.rust-lang.org/stable/std/index.html)
- [Result Enum](https://doc.rust-lang.org/stable/core/result/enum.Result.html)
- [Result IntoIter](https://doc.rust-lang.org/stable/core/result/struct.IntoIter.html)
- [Result Iter](https://doc.rust-lang.org/stable/core/result/struct.Iter.html)
- [Result IterMut](https://doc.rust-lang.org/stable/core/result/struct.IterMut.html)

### Error Handling Philosophy:
> "Panics are meant for unrecoverable errors" - Official Rust Documentation  
> "Generally discouraged" - Official stance on unwrap()/expect() usage  
> "Use the `?` operator for elegant error propagation" - Recommended pattern  

---

**🎯 Zero-Tolerance Rule**: This document defines hard requirements based on official Rust documentation. All violations block CI/CD and prevent deployment. Agents MUST validate against these rules before generating any code.