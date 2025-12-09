# ❌ Rust Production Anti-Patterns — Zero Tolerance (2025, Rust 1.91.1)

> **Purpose**: Hard rules for production-grade high-load systems based on official Rust stdlib documentation  
> **Sources**: https://doc.rust-lang.org/stable/std/ + performance research + production analysis  
> **Updated**: January 2025 based on Rust 1.90+ stable documentation (addenda for Rust 1.91.1 patch release[^rust1911])  
> **Enforcement**: clippy/scripts/CI + mandatory agent compliance  

---

## 0) Rust 1.91.1 Toolchain & Platform Guardrails

### 🎯 Mandatory Toolchain Pinning

- CI/build images **must** pin `rustc`/`cargo` 1.91.1 to pick up the Wasm import fix and illumos locking patch.[^rust1911]
- `rust-toolchain.toml` stays on `channel = "1.91.1"` (repo policy already forbids `rust-version` inside Cargo manifests).
- Add `make toolchain-verify`: fails if `rustc --version` output lacks `1.91.1`.
- Release checklist: `rustup run 1.91.1 cargo metadata --locked` before tagging; fail if toolchain drifts.

### 🛰️ Wasm Import Regression Guardrail

- Rust 1.91.0 had UB when multiple crates used `#[link(wasm_import_module)]`; 1.91.1 fixes it.[^rust1911]
- Never build wasm-astro (or any Wasm target) on <1.91.1.
- Add `scripts/test-wasm-imports.sh`: links two dummy crates with unique `wasm_import_module` values to catch regressions; run in CI.

### ☀️ Cargo Target Locking on illumos / Shared Storage

- 1.91.1 restores `File::lock` so Cargo can serialize writes; older toolchains corrupt `target/` on parallel builds.[^rust1911]
- Guardrail:
  - On illumos/network filesystems run smoke test `cargo check & cargo check` (expect success).
  - Detection script should fall back to per-job `CARGO_TARGET_DIR` if locks are unsupported.

### 📚 Documentation & Plan Sync

- `anti.md` is authoritative; after merging content from other guidance (e.g., `rust-1.91.1-production-anti-patterns.md`), that file may be deleted.
- When new Rust releases land, append a subsection here summarizing critical fixes + enforcement hooks.

---

## 1) ERROR HANDLING: Zero-Tolerance Production Rules

### 🚫 ABSOLUTELY FORBIDDEN

Based on official Result enum documentation:

- **`unwrap()`, `expect()`, `panic!()`** — "Generally discouraged" per stdlib docs, kills availability
- **`unwrap_or(..)` / `unwrap_or_else(..)`** — eager default evaluation masks failures
- **Custom unwrap variants** — `unwrap_u64`, `unwrap_str`, etc. - explicit type checking required
- **Error suppression** — Ignoring `Result`/`Option` without explicit handling violates Rust principles
- **Panic in libraries** — "Panics are meant for unrecoverable errors" - forces termination on users

🔁 **Allowed narrow case**: `unwrap_or_default()` is acceptable *only* for trivially safe, zero-cost defaults (string, numeric, bool) where the success arm is identity and no side effects occur—matching repo policy. Everything else uses `match`/`if let`.

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

- unwrap_or_default() — **only** for trivial zero-cost defaults:
```rust
// ✅ Allowed (string/number/bool, identity success arm, no side effects)
let name: String = maybe_name.unwrap_or_default();
// ❌ Use match if fallback allocates/does business logic
let cfg = match fetch_cfg() {
    Ok(v) => v,
    Err(_) => build_cfg(), // fallback does work, so no unwrap_or_default
};
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

### ⏱️ REAL-TIME LATENCY & BOUNDED CONCURRENCY (Rust 1.91.1+)

- **Budget every hop**: define p95/p99 latency targets per async pipeline; fail CI benches exceeding budgets (Tokio recommends explicit budgets for cooperative scheduling[^tokio]).
- **Cap fan-out**: prefer `.buffered(N)` / `.buffer_unordered(N)` / `tokio::sync::Semaphore` to keep concurrency bounded.
- **Yield manually**: CPU-bound loops call `tokio::task::yield_now().await` every ≤500 µs to keep runtimes responsive.
- **Back-pressure channels**: use bounded `mpsc::channel(cap)` to avoid unbounded allocations; dropping messages requires explicit policy.
- **Watchdogs**: expose heartbeat metrics (`tokio-metrics`, `tracing` spans) and fail readiness if loop lag > budget.

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

## 8) DOCUMENTATION & SAMPLE CODE HYGIENE

- **No `unwrap`/`expect` in docs**: Rust API Guidelines require using `?` in examples so downstream users aren’t trained to panic.[^api]
- Run `cargo test --doc` under `cargo clippy -- -D clippy::panic -D clippy::unwrap_used`.
- README/tutorial snippets must compile and use the same structured error handling as production code; reviewers reject samples that deviate.

## 9) UNSAFE / FFI CONTRACTS

- Every `unsafe` block documents invariants and references the relevant Rustonomicon chapter.[^nomicon]
- `extern "C"` functions spell out ownership/lifetime expectations and are wrapped behind safe APIs before exposure.
- Manual `Send`/`Sync` impls require justification plus tests (loom, concurrency stress) proving correctness.
- CI tracks `unsafe` deltas via `cargo geiger`; PR reviewers must sign off on any increase.

## 10) AGENT COMPLIANCE: Mandatory Rules

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

## 11) ENFORCEMENT: Quality Gates

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
- **Toolchain compliance** - `make toolchain-verify` fails if `rustc` ≠ 1.91.1
- **Wasm import regression test** - `scripts/test-wasm-imports.sh` runs in CI for wasm-astro

---

## References (Official Documentation)

### Primary Sources:
- [Rust Standard Library](https://doc.rust-lang.org/stable/std/index.html)
- [Result Enum](https://doc.rust-lang.org/stable/core/result/enum.Result.html)
- [Result IntoIter](https://doc.rust-lang.org/stable/core/result/struct.IntoIter.html)
- [Result Iter](https://doc.rust-lang.org/stable/core/result/struct.Iter.html)
- [Result IterMut](https://doc.rust-lang.org/stable/core/result/struct.IterMut.html)
- [Rustonomicon](https://doc.rust-lang.org/nomicon/)
- [Tokio Topics: Bridging with sync code](https://tokio.rs/tokio/topics/bridging)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/documentation.html)
- [Announcing Rust 1.91.1](https://blog.rust-lang.org/2025/11/10/Rust-1.91.1/)

### Error Handling Philosophy:
> "Panics are meant for unrecoverable errors" - Official Rust Documentation  
> "Generally discouraged" - Official stance on unwrap()/expect() usage  
> "Use the `?` operator for elegant error propagation" - Recommended pattern  

---

**🎯 Zero-Tolerance Rule**: This document defines hard requirements based on official Rust documentation and the Rust 1.91.1 release guidance. All violations block CI/CD and prevent deployment. Agents MUST validate against these rules before generating any code.

[^api]: Rust API Guidelines, checklist item C-QUESTION-MARK — examples should prefer `?` over `unwrap`.
[^tokio]: Tokio Topics “Bridging with sync code” — explains yielding and bounded blocking in cooperative schedulers.
[^nomicon]: *The Rustonomicon*, chapters “Ownership and Borrowing” / “Send and Sync”.
[^rust1911]: *Announcing Rust 1.91.1*, Rust Blog (2025‑11‑10) — fixes Wasm import-module regression and re-enables `File::lock` on illumos.
