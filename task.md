Title: StarsCalendars — Astronomical Scene, WASM, Backend, Quality (current)
Selected from tasks-list.md
# Title: StarsCalendars — Astronomical Scene, WASM, Backend, Quality (current)

## Goal
Astronomical precision hardening and Quantum Time (NT) migration to WASM for reuse across events.

## Subtasks
- [x] Add reusable UTC↔TT timescales module with leap seconds (TAI−UTC + 32.184s) and optional override setters
- [x] Improve winter solstice timing: use λ_app (FK5 + aberration + nutation) and Newton solver for λ=270° (TT), convert to UTC
- [x] Expand coarse path elimination; replace day-scan/ternary with Newton (≈20× faster)
- [x] Expose `get_quantum_time_components(epoch_ms, tz_offset_min)` from WASM; remove JS NT logic; update UI to minute cadence
- [x] Reuse sublunar computation once per frame for moon direction and marker
- [x] Fix GUI clipping of NT/current time labels (increase height, vertical centering)
- [ ] Add symmetric helpers: `next_summer_solstice_from`, `next_vernal_equinox_from`, `next_autumnal_equinox_from`
- [ ] Unit tests: compare event times 2023–2027 to reference (tolerance ≤ 10 s)

## Done Criteria
- Winter solstice time within seconds vs reference sources (typ. 2–10 s)
- Timescales utilities used by all event timing functions
- NT label sourced from WASM and updates once per minute without per-frame cost
- Scene uses one sublunar computation per frame

---

## Parallel Goal (Quality): Ban unwrap_* variants repo-wide and wire CI to fail on detection

## Subtasks
- [x] Update `scripts/anti-patterns.sh` to detect: `unwrap_\w+`, `unwrap_err`, `unwrap_unchecked`, `unwrap_u8`, and custom helpers like `*_unwrap*`
- [ ] Add `.githooks/pre-commit` hook that runs `make anti-patterns` and blocks on violations
- [ ] Ensure GitHub Actions job runs anti-pattern scan on every PR (Quality Guardian workflow)
- [ ] Run repository-wide scan and fix any findings (prod code only; tests may use `.expect()` with messages)
- [ ] Document rules in `QUALITY.md` and link from `README.md`

## Roadmap (structured, newcomer-ready)

### A. WASM astro core (state=15, zero-copy, one call per frame)
- [x] STATE layout 15 f64: Sun zeros[0..2], Moon dist AU[3], Earth RA/Dec/dist AU[4..6], Solar zenith lon/lat[7..8], sublunar lat/lon[9..10], Earth-local Moon unit vector[11..13], AST rad[14].
- [x] Thread-local buffer, zero-copy Float64Array view.
- [x] Sun slots zeroed; heliocentric scene; no Sun transform updates.
- [x] Timescales module UTC↔TT via (TAI−UTC)+32.184s; leap seconds table; override setters.
- [x] `next_winter_solstice_from` λ_app=270° (FK5+aberration+nutation, TT→UTC).
- [ ] Add `next_summer_solstice_from`, `next_vernal_equinox_from`, `next_autumnal_equinox_from` (reuse solver).
- [ ] Tests 2023–2027 events (≤10 s tolerance).
- [ ] Extend STATE with lunar RA/Dec explicitly if not already surfaced (doc cross-check).

### B. Frontend Babylon.js scene (React 19, TS 5.9, Babylon 8)
- [x] Single `compute_state(jd)` per frame; reuse view; no allocations in render loop.
- [x] RH→LH single Z-flip in scene only; Babylon left-handed; no `useRightHandedSystem`.
- [x] Use STATE[15]: Moon dist AU, Earth RA/Dec/dist, zenith lon/lat, sublunar lat/lon, Earth-local Moon vector, AST.
- [x] Sun fixed at origin; materials frozen; godrays ratio=1.0 (quality).
- [ ] Visual tidal lock for Moon using Earth→Moon vector.
- [ ] Performance pass: ensure 60 FPS target (profiling); confirm zero GC in loop.

### C. Backend (Axum, SQLX, Telegram, JWT RS256)
- [x] JWT service uses generated RSA keys in dev/test (no embedded PEM); prod loads from secure storage.
- [ ] Add RS256 validation presence check (security gate) wiring.
- [ ] Ensure `teloxide` flows documented; subscription verification cached.
- [ ] SQLX offline/online workflow documented; indices per tz.md.

### D. Quality/CI tooling
- [x] Anti-patterns script: absolute ban on all `unwrap*`/`expect*` (incl. unwrap_or_default), tests included.
- [x] Secret scan widened (bearer/URL creds, token filter, .claude exclude).
- [x] Manifest/versions/tasks guards added.
- [ ] Pre-commit hook to run `make anti-patterns`.
- [ ] GitHub Actions job runs anti-patterns + secret-scan + manifest/versions guard.
- [ ] cargo-deny/cargo-audit wired in CI (deny.toml aligned).

### E. Documentation (newcomer-complete)
- [ ] README: current STATE[15], scene integration steps, RH→LH flip, single-call rule, dev RSA generation.
- [ ] tz.md: sync with STATE[15], zero-copy, one-call; event helpers; quality gates.
- [ ] QUALITY.md: ban unwrap_or_default; RSA policy; scene/WASM rules.
- [ ] CLAUDE.md / .cursorrules: same policies; edition 2024, no rust-version, majors-only pins.
- [ ] Agents in `.claude/agents`: updated with STATE[15], unwrap ban, RSA generation notes.

### F. Tasks/Plans hygiene
- [ ] Keep `task.md` Title present; ensure tasks-guard passes.
- [ ] Expand tasks-list.md with full project roadmap (above sections), synced.

## Done Criteria
- CI fails on `unwrap_u8`/`unwrap_unchecked`/`unwrap_err` (and any `unwrap_*`) usages
- Repository scan is clean (no production usages); tests follow CLAUDE.md allowances
