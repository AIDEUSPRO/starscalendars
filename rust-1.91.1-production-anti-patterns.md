# Rust 1.91.1 Production Anti-Patterns & Guardrails

## Scope & methodology

- **Version focus:** Rust 1.91.1 (released 2025‑11‑10) is a point release over 1.91.0 that fixes a Wasm import regression and restores `File::lock` support on illumos.[^rust1911] The anti-patterns below assume this exact toolchain and highlight practices that negate the fixes bundled with 1.91.1.
- **Primary sources:** Official Rust Book guidance on panics, the Rust API Guidelines checklist, The Rust Performance Book, and Tokio’s runtime documentation. These are the canonical references for safe, idiomatic, and high-performance Rust.
- **Structure:** Each anti-pattern maps the risky behavior to the relevant doc excerpt, explains why it is especially harmful in 1.91.1-era systems, and lists a production-ready remediation.

## Quick reference

| # | Anti-pattern | Why it breaks 1.91.1-grade systems | Production guardrail |
|---|--------------|------------------------------------|----------------------|
| 1 | Staying on 1.91.0 after the Wasm/Cargo regressions | Leaves known undefined behavior in Wasm symbol resolution and disables Cargo target locking on illumos, risking corruption during parallel builds | Pin CI/CD images to 1.91.1 immediately; add release-tracking in `rust-toolchain.toml` or infra manifests[^rust1911] |
| 2 | Treating `panic!/unwrap/expect` as normal control flow | Forcibly aborts callers that could otherwise recover; violates the Book’s guidance to return `Result` by default and only panic on invariant violations[^rustbook] | Propagate errors with `?`, domain error enums, and telemetry; reserve `panic!` for “bad state” invariants only |
| 3 | Shipping examples/tests that normalize `unwrap` | API Guidelines call for `?` in examples so users are not trained to panic; ignoring that leads downstream teams to copy unsafe snippets[^api] | Gate sample code through Clippy’s `pedantic`/`restriction` sets; enforce review checklist entries for documentation snippets |
| 4 | Blocking sync workloads directly inside async tasks | Tokio docs insist on isolating sync work (e.g., via `spawn_blocking` or dedicated runtimes); blocking inside `.await` paths can starve the scheduler and inflate tail latency[^tokio] | Fence legacy sync calls behind `spawn_blocking` or `Runtime::block_on` helpers; document the bridging strategy per component |
| 5 | Ignoring Clippy/perf lint automation | The Rust Performance Book emphasizes Clippy’s Perf lints for catching suboptimal patterns automatically; skipping them leaves “slow by default” code paths[^perfbook] | Run `cargo clippy --all-targets --all-features -D warnings` in CI; track `clippy.toml` with `disallowed_types` for perf-sensitive modules |
| 6 | Running concurrent Cargo builds without target locking | 1.91.0 broke `File::lock` on illumos, so skipping the 1.91.1 fix reintroduces race conditions between parallel builds[^rust1911] | After upgrading, keep `CARGO_TARGET_DIR` on supported filesystems and add smoke tests that spawn simultaneous `cargo check` invocations |

## Detailed guidance

### 1. Lagging patch-level toolchains
Rust 1.91.1 explicitly fixes two regressions introduced in 1.91.0: a Wasm import-module mismatch that could trigger UB at runtime, and the `File::lock` API returning `Unsupported` on illumos, disabling Cargo’s build-directory locks.[^rust1911] Shipping mission-critical binaries on 1.91.0 (or older) leaves those bugs unfixed—meaning prototypical “parallel release builds” or multi-crate Wasm projects can silently corrupt memory or clobber build artifacts.

**Guardrail:** bake `rustup toolchain install 1.91.1` into CI images, set `channel = "1.91.1"` in `rust-toolchain.toml`, and add a regression test that runs two `cargo check` commands concurrently on illumos runners to prove locking is functional.

### 2. Unrecoverable panic surfaces
The Rust Book reiterates that returning `Result` should be the default because it gives callers the freedom to recover, while `panic!` should be reserved for unrecoverable “bad state” violations.[^rustbook] Library code that calls `unwrap` or `expect` effectively decides on behalf of every consumer that failure is fatal. In 1.91.1-era services (especially high-load telemetry systems) that translates to cascading outages when a single I/O error bubbles up.

**Guardrail:** model every fallible boundary with error enums or `thiserror`-style types, propagate with `?`, and centralize fatal assertions behind debug-only invariants. Reserve `panic!` for genuinely corrupted invariants (e.g., unsafe contracts or memory corruption) and ensure telemetry distinguishes intentional aborts from bugs.

### 3. Teaching clients to panic
The Rust API Guidelines’ C-QUESTION-MARK item explicitly calls for using `?`—not `try!` or `unwrap`—in examples so that consuming teams aren’t normalized to panicking patterns.[^api] Docs, READMEs, and snippets that ship with `unwrap()` effectively bake an anti-pattern into downstream code, especially when teams copy/paste example workloads into production.

**Guardrail:** add documentation tests (`rustdoc --test`) that use `?` paths, lint documentation with `cargo clippy --all-targets -- -D clippy::unwrap_used`, and enforce peer-review checklist items that reject `unwrap` in snippets unless the section explicitly discusses panics.

### 4. Blocking in async runtimes
Tokio’s “Bridging with sync code” topic states plainly that synchronous work should be isolated—either via `spawn_blocking`, `Runtime::block_on`, or by running a dedicated runtime alongside the GUI/main thread.[^tokio] Dropping CPU-bound or blocking syscalls into async tasks prevents the scheduler from polling other futures, which shows up as tail-latency spikes and watchdog resets in real-time services.

**Guardrail:** wrap every synchronous FFI or filesystem call in a `spawn_blocking` helper, codify the bridging strategy per crate (e.g., “GUI thread owns sync, worker runtime handles async”), and add integration tests that assert event loops remain responsive under artificial blocking loads.

### 5. Skipping lint & perf automation
Nick Nethercote’s Rust Performance Book dedicates an entire chapter to linting, emphasizing that Clippy catches many performance problems automatically and should be part of every pipeline.[^perfbook] Teams that disable or ignore Clippy perf lints routinely ship code with needless heap allocations, suboptimal container choices, or misuse of expensive APIs—anti-patterns that erode 1.91.1’s performance gains.

**Guardrail:** run `cargo clippy --all-targets --all-features -D warnings -W clippy::perf` in CI, maintain a `clippy.toml` that declares `disallowed-types` (e.g., forbid `std::collections::HashMap` where `hashbrown` is mandated), and track regressions with perf baselines per microservice.

### 6. Unlocked target directories on illumos
The 1.91.1 release notes highlight that 1.91.0’s switch to `File::lock` inadvertently returned `Unsupported` on illumos, causing Cargo to skip locking `target/` entirely.[^rust1911] Any build farm that parallelizes `cargo build` on shared storage becomes vulnerable to corrupted artifacts or racey incremental caches.

**Guardrail:** ensure build hosts have been updated to 1.91.1+, run a diagnostic that inspects `File::lock` on illumos at startup, and, when necessary, serialize `cargo` invocations or isolate `CARGO_TARGET_DIR` by job to avoid shared-state corruption.

## Operational checklist

1. **Toolchain hygiene:** Track release notes and automate `rustup update stable` in staging to catch regressions early (escrow channel switches before production).
2. **Static enforcement:** Bake Clippy perf and panic bans into CI; fail builds on `panic!/unwrap/expect` outside test modules.
3. **Runtime isolation:** Document for every binary whether sync work is bridged via `spawn_blocking`, dedicated runtimes, or process boundaries; regression-test that loops stay responsive.
4. **Docs-as-code:** Lint documentation snippets, and avoid publishing examples that model anti-patterns.
5. **Parallel build safety:** On illumos or network filesystems, smoke-test concurrent `cargo` invocations to guarantee locking works post-upgrade.

## Source index

- [^rust1911]: *Announcing Rust 1.91.1*, Rust Blog (2025‑11‑10) — fixes Wasm import-module mismatch and re-enables `File::lock` on illumos.
- [^rustbook]: *The Rust Programming Language*, Chapter 9.3 “To panic! or Not to panic!” — recommends returning `Result` by default and reserving `panic!` for unrecoverable bad states.
- [^api]: *Rust API Guidelines*, checklist item C-QUESTION-MARK (examples should use `?`, not `unwrap`) — discourages normalizing panics in documentation.
- [^perfbook]: *The Rust Performance Book*, “Linting” chapter — Clippy perf lints catch common performance issues automatically and should run in all pipelines.
- [^tokio]: *Tokio Topics: Bridging with sync code* — advises using `spawn_blocking` or dedicated runtimes when mixing synchronous workloads with async code.

