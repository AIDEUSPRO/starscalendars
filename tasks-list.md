# Tasks List (high-level, newcomer-complete)

## 🔧 КАК СОЗДАТЬ WASM ОБЕРТКУ С НУЛЯ

### Цель
Обертка предоставляет астрономические данные для 3D сцены через единый `compute_state(jd)`.

### Алгоритм создания
1. **Изучить astro-rust API** в `./astro-rust/src/`: sun, lunar, planet, nutation, precess, time, ecliptic, coords
2. **Создать thread-local буфер** `[f64; N]` для zero-copy
3. **Вычислить общие величины один раз**: nutation, obliquity, AST
4. **Заполнить буфер** используя ТОЛЬКО astro-rust функции
5. **Вернуть указатель** `out.as_ptr()` — JS создает Float64Array view

### Текущий STATE layout (27 f64, append-only)
| Slots | Данные | Назначение для сцены |
|-------|--------|---------------------|
| [0..2] | Sun zeros | Солнце статично в (0,0,0) |
| [3] | Moon dist AU | Масштабирование орбиты Луны |
| [4..6] | Earth RA/Dec/dist | Позиция Земли вокруг Солнца |
| [7..8] | Zenith lon/lat | Ориентация earthPivot |
| [9..10] | Sublunar lat/lon | Зеленый маркер на Земле |
| [11..13] | Moon direction | Единичный вектор Земля→Луна |
| [14] | AST | Apparent sidereal time |
| [15] | Sun ecl long | Zodiac/events (apparent: FK5+aberr+nut) |
| [16] | Moon ecl long | Zodiac/events (with nutation) |
| [17] | Moon ecl lat | Events (e.g. eclipses classifier) |
| [18] | Moon illum frac | UI (% illuminated) |
| [19] | Moon–Sun elong | UI/events (phase angle) |
| [20..23] | Zodiac indices | Sun/Moon tropical + sidereal(MVP=J2000) |
| [24] | Moon asc node long | Nodes/events |
| [25] | Moon mean perigee long | Apsides/events |
| [26] | Moon phase8 id | UI (0..7) |

### При расширении
- Добавлять в конец, не менять существующие индексы
- Синхронизировать: tz.md, README.md, CLAUDE.md, .cursorrules, init.ts, BabylonScene.tsx, agents

---

- [ ] A. WASM astro core (state=15, zero-copy, one call per frame)
  - [x] STATE base layout 15 f64 (indices 0..14) implemented and stable
  - [ ] STATE extended layout 27 f64 (append-only indices 15..26) for zodiac/events (see table above)
  - [x] Thread-local buffer, zero-copy Float64Array view
  - [x] Sun slots zeroed; heliocentric scene; no Sun transform updates
  - [x] Timescales module UTC↔TT via (TAI−UTC)+32.184s; leap seconds table; override setters
  - [x] `next_winter_solstice_from` λ_app=270° (FK5+aberration+nutation, TT→UTC)
  - [ ] Add `next_summer_solstice_from`, `next_vernal_equinox_from`, `next_autumnal_equinox_from`
  - [ ] Tests 2023–2027 events (≤10 s tolerance)
  - [ ] Confirm/extend STATE with lunar RA/Dec if needed (doc parity)
  - [ ] Zodiac + Lunar events: extend STATE by appending new slots (do not change existing indices); export off-frame event helpers (phases/nodes/apsides/eclipses/void-of-course)

- [ ] B. Frontend Babylon.js scene (React 19, TS 5.9, Babylon 8)
  - [x] Single `compute_state(jd)` per frame; reuse view; no allocations in render loop
  - [x] RH→LH single Z-flip in scene only; Babylon left-handed; no `useRightHandedSystem`
  - [x] Consume STATE base slots [0..14]: Moon dist AU, Earth RA/Dec/dist, zenith lon/lat, sublunar lat/lon, Earth-local Moon vector, AST
  - [ ] Consume STATE appended slots [15..26] for zodiac/lunar events UI
  - [x] Sun fixed at origin; materials frozen; godrays ratio=1.0
  - [x] Camera presets Earth↔Moon: robust two-phase ArcRotate apply + reset/lock limits (Earth preset on startup and 🌍 button; Moon preset on 🌙 button)
  - [ ] LunarInfoPanel near the Moon (Babylon GUI, `linkWithMesh`), shown only in moon camera mode
  - [x] add “РАСШИФРОВКА ДНЯ в @elioncalendar” inside scene top on quantum time
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
  - [ ] README: STATE[27] (append-only), scene steps, RH→LH flip, single-call rule, dev RSA generation
  - [ ] tz.md: sync with STATE[27] (append-only), zero-copy, one-call; event helpers; quality gates
  - [ ] QUALITY.md: ban unwrap_or_default; RSA policy; scene/WASM rules
  - [ ] CLAUDE.md / .cursorrules: same policies; edition 2024, no rust-version, majors-only pins
  - [ ] Agents `.claude/agents`: updated with STATE[27], unwrap ban, RSA generation notes

- [ ] F. Tasks/Plans hygiene
  - [ ] Keep `task.md` Title present; tasks-guard passes
  - [ ] Keep tasks-list.md synced with roadmap above

- [ ] Astronomical/Spiritual Events
  - [ ] Orion–SÜN alignment (Tatev): implement `next_orion_alignment_from(jd_utc_start, lat_rad, lon_east_rad)` in WASM; define belt PA/azimuth target, use RA/Dec of Alnitak/Alnilam/Mintaka with proper motion; solve time via Newton. Cache in frontend; update GUI once per minute
  - [ ] NT integration: сделать старт квантового года = событие синхронизации Ориона с СЮН; рефактор NT так, чтобы базовая эпоха приходила от провайдера событий (fallback — текущая constNT)
