# Tasks List (high-level, newcomer-complete)

- [ ] A. WASM astro core (state=15, zero-copy, one call per frame)
  - [x] STATE layout 15 f64: Sun zeros[0..2], Moon dist AU[3], Earth RA/Dec/dist AU[4..6], Solar zenith lon/lat[7..8], sublunar lat/lon[9..10], Earth-local Moon unit vector[11..13], AST rad[14]
  - [x] Thread-local buffer, zero-copy Float64Array view
  - [x] Sun slots zeroed; heliocentric scene; no Sun transform updates
  - [x] Timescales module UTC↔TT via (TAI−UTC)+32.184s; leap seconds table; override setters
  - [x] `next_winter_solstice_from` λ_app=270° (FK5+aberration+nutation, TT→UTC)
  - [ ] Add `next_summer_solstice_from`, `next_vernal_equinox_from`, `next_autumnal_equinox_from`
  - [ ] Tests 2023–2027 events (≤10 s tolerance)
  - [ ] Confirm/extend STATE with lunar RA/Dec if needed (doc parity)

- [ ] B. Frontend Babylon.js scene (React 19, TS 5.9, Babylon 8)
  - [x] Single `compute_state(jd)` per frame; reuse view; no allocations in render loop
  - [x] RH→LH single Z-flip in scene only; Babylon left-handed; no `useRightHandedSystem`
  - [x] Consume STATE[15]: Moon dist AU, Earth RA/Dec/dist, zenith lon/lat, sublunar lat/lon, Earth-local Moon vector, AST
  - [x] Sun fixed at origin; materials frozen; godrays ratio=1.0
  - [ ] Visual tidal lock for Moon using Earth→Moon vector
  - [ ] Performance pass: 60 FPS target, zero GC in loop

- [ ] C. Backend (Axum, SQLX, Telegram, JWT RS256)
  - [x] Dev/test JWT RSA keys generated on the fly (rsa+rand); no embedded PEM
  - [ ] RS256 validation presence gate in security checks
  - [ ] Teloxide flow documented; subscription verification cached
  - [ ] SQLX offline/online workflow documented; indices per tz.md

- [ ] D. Quality/CI tooling
  - [x] Anti-patterns: absolute ban on all `unwrap*`/`expect*` (incl. unwrap_or_default), tests included
  - [x] Secret scan widened (bearer/URL creds, token filter, .claude exclude)
  - [x] Manifest/versions/tasks guards added
  - [ ] Pre-commit hook runs `make anti-patterns`
  - [ ] GitHub Actions runs anti-patterns + secret-scan + manifest/versions guard
  - [ ] cargo-deny/cargo-audit wired in CI (deny.toml aligned)

- [ ] E. Documentation (newcomer-complete)
  - [ ] README: STATE[15], scene steps, RH→LH flip, single-call rule, dev RSA generation
  - [ ] tz.md: sync with STATE[15], zero-copy, one-call; event helpers; quality gates
  - [ ] QUALITY.md: ban unwrap_or_default; RSA policy; scene/WASM rules
  - [ ] CLAUDE.md / .cursorrules: same policies; edition 2024, no rust-version, majors-only pins
  - [ ] Agents `.claude/agents`: updated with STATE[15], unwrap ban, RSA generation notes

- [ ] F. Tasks/Plans hygiene
  - [ ] Keep `task.md` Title present; tasks-guard passes
  - [ ] Keep tasks-list.md synced with roadmap above

- [ ] Astronomical/Spiritual Events
  - [ ] Orion–SÜN alignment (Tatev): implement `next_orion_alignment_from(jd_utc_start, lat_rad, lon_east_rad)` in WASM; define belt PA/azimuth target, use RA/Dec of Alnitak/Alnilam/Mintaka with proper motion; solve time via Newton. Cache in frontend; update GUI once per minute
  - [ ] NT integration: сделать старт квантового года = событие синхронизации Ориона с СЮН; рефактор NT так, чтобы базовая эпоха приходила от провайдера событий (fallback — текущая constNT)
