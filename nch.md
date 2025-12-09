## Next‑chat bootstrap (engineer notes)

Purpose: snapshot of what’s done, how to rebuild headspace fast, and the exact next actions to take in a fresh session.

### What’s already implemented (high‑signal)
- Event timing accuracy (WASM): winter solstice via λ_app(t)=270° with FK5 + annual aberration + nutation, solver Newton with guarded step; TT↔UTC conversion via timescales (TAI−UTC + 32.184 s). Returns JD UTC.
- Solar zenith for frame hot path: now also uses FK5 + aberration + nutation (apparent), consistent with event physics.
- NT (Quantum Time) migrated to WASM: `get_quantum_time_components(epoch_ms, tz_offset_minutes)` → three f64 values; minute‑cadence GUI update; JS NT removed.
- Sublunar/scene: one computation per frame reused for moon vector and marker.
- Safari/WebGL stability: skybox now avoids early bind; clouds texture NPOT without mipmaps; cubemap noMipmap=true.
- Pre‑commit/quality:
  - Hardcoded constants gate refined: allow markers `@allow-wasm-const` (project non‑astro constants) and `@allow-numeric-param` (solver numeric params). Everything else still blocked.
  - Multiple WASM calls check is disabled in local hook by env (ALLOW_MULTIPLE_WASM_CALLS=1); CI can keep it strict.
  - `fmt` no longer formats `astro-rust/`; formatting runs per‑crate only.

### Quick rebuild context (how to run)
- Frontend dev (includes WASM build):
  - `pnpm -w run dev:frontend-only`
- WASM build only: 
  - `pnpm run build:wasm`
- Hooks:
  - configured via simple-git-hooks; to re‑install: `pnpm run prepare`

### Constraints / rules we keep in mind
- astro‑rust/ is READ‑ONLY; all astronomical physics must come from astro‑rust API. Own constants allowed only when not astronomical (NT constants, numeric solver params) and must be marked with `@allow-wasm-const` / `@allow-numeric-param`.
- One compute_state(jd) per frame hot path; additional WASM calls only off‑frame (minute cadence, idle callbacks).
- Zero‑copy Float64Array usage; no strings across WASM boundary in prod.

### Immediate next actions (in order)
1) Remove TS trigonometry; provide radians directly from WASM
   - Add in `wasm-astro/src/lib.rs`:
     - `#[wasm_bindgen] pub fn get_sublunar_rad(jd: f64) -> *const f64` → `[lon_east_rad, lat_rad]`.
     - `#[wasm_bindgen] pub fn get_lunar_ra_dec_ast(jd: f64) -> *const f64` → `[ra_rad, dec_rad, ast_rad]` (apparent sidereal time; same obliquity/ nutation path as zenith).
     - Optionally expose Earth‑local unit vector Earth→Moon to remove remaining scene math.
   - TS: in `BabylonScene.tsx` replace `computeSublunarLatLonDeg` with WASM call; drop degree↔radian conversions and custom trig.
   - Keep hot path at 1× compute_state; the extra helper is read only when minute tick (or once per frame if we decide to fold into STATE later).

2) Seasonal events parity (re‑use λ_app solver)
   - Implement in WASM: 
     - `next_summer_solstice_from(jd_utc_start)` (λ_app=90°),
     - `next_vernal_equinox_from(jd_utc_start)` (λ_app=0°),
     - `next_autumnal_equinox_from(jd_utc_start)` (λ_app=180°).
   - Dev test harness (debug‑only): compare 2023–2027 times to reference within ≤ 10 s; log to console.

3) NT epoch provider from Orion–SÜN alignment (Tatev)
   - WASM: `next_orion_alignment_from(jd_utc_start, lat_rad, lon_east_rad) -> f64` (JD UTC).
     - Stars: belt (Alnitak/Alnilam/Mintaka) RA/Dec with proper motions; compute target condition (to be finalized: azimuth/altitude or position angle alignment vs SÜN direction). Newton solve; TT↔UTC via timescales.
   - NT integration: refactor NT so baseline epoch comes from provider (fallback to current constNT) and recalc once per minute/cached.

4) Optional: Extended STATE (when we want TS hot path absolutely trig‑free)
   - Either expand STATE (new parallel buffer) with: `lunar_ra_rad`, `lunar_dec_rad`, `apparent_sidereal_time_rad`, `sublunar_lon_east_rad`, `sublunar_lat_rad`, `earth_to_moon_unit_local[3]`.
   - Or keep helpers off‑frame; hot path remains 11 f64.

### Quality/CI switches
- Local hook already sets `ALLOW_MULTIPLE_WASM_CALLS=1` (GUI‑safe). In CI leave unset for strict mode, or set only for jobs that run minute‑tick code.
- Mark any future allowed constants with `@allow-wasm-const` (project logic) or `@allow-numeric-param` (alg params) to satisfy `wasm-critical`.

### Code pointers (fast jump)
- WASM zenith (now FK5+aberration+nutation): `wasm-astro/src/lib.rs` → `solar_zenith_position_rad_internal`.
- Event solver λ_app: same file, `next_winter_solstice_from`. Reuse path for new seasonal events.
- Timescales module (UTC↔TT): `wasm-astro/src/lib.rs` → `mod timescales` (`utc_to_tt_jd`, `tt_to_utc_jd`, leap seconds + override setters).
- Frontend WASM API: `frontend/src/wasm/init.ts` (typed interface + memory access).
- Scene updates (minute cadence solstice + NT): `frontend/src/scene/BabylonScene.tsx`.
- Quality rules: `Makefile` targets `wasm-critical`, `wasm-perf`, `fmt`; allowed markers wired in.

### Frame alignment analysis (Earth/starframe inversion bug)

Observed (user): around late September (autumnal equinox), Earth appears on the wrong side relative to the stellar background; constellations are correct, green sublunar marker and Moon placement are correct. Root cause is most likely a scene frame mismatch between the stellar sphere construction (RA/Dec → Babylon) and our ecliptic heliocentric mapping for Earth (and optionally Moon), not a WASM sign error.

Current mappings in code:
- Stellar sky (`createSky` in `BabylonScene.tsx`): stars are placed by rotating a vertex at +Z using `Matrix.RotationYawPitchRoll(-RA, -Dec, 0)`. Therefore:
  - RA = 0 → +Z
  - RA = 90° → +X
  - RA = 180° → -Z
  - RA = 270° → -X
- Earth position (heliocentric ecliptic rectangular from WASM): currently mapped as
  - scene.x = ecliptic.x
  - scene.y = ecliptic.z
  - scene.z = -ecliptic.y

Implication: ecliptic long λ = 0° (Aries) lies along +X in our Earth mapping, but along +Z in the star sphere. This frame mismatch explains why Earth seems on the “wrong” side vs constellations near equinoxes (user reports starting late September).

Green sublunar marker and Moon are fine because they are computed in Earth-local coordinates using RA/Dec/AST math and then transformed by Earth pivot orientation; they do not depend on the global ecliptic→scene mapping used for Earth world position.

Proposed fix (unify frames): map heliocentric ecliptic axes to Babylon so that Aries (λ=0) → +Z, consistent with stars.
- Recommended mapping for Earth (and for Moon when using raw vectors):
  - scene.x = -ecliptic.y
  - scene.y = +ecliptic.z
  - scene.z = +ecliptic.x

This sets: x_ecl→+Z_scene, y_ecl→-X_scene, z_ecl→+Y_scene, matching the star sphere convention (RA 0 at +Z). Babylon is left-handed; this mapping preserves orientation for our usage.

Implementation steps (do not apply until explicitly requested):
1) In `updateCelestialPositionsRealtime`, change Earth world position to:
   - `earthPositionVector.set(-ey * scaleAU, ez * scaleAU, ex * scaleAU)` where `(ex,ey,ez)` are heliocentric ecliptic from STATE.
2) For Moon placement if using raw geocentric vector (not the current Earth‑local sublunar path), use the same mapping for consistency:
   - `moonMesh.position.set(-myUnits, mzUnits, mxUnits)`.
3) Keep all Earth‑local calculations (zenith marker; sublunar marker; pivot orientation) unchanged; they operate in the local sphere frame and remain correct.
4) Add a tiny debug harness (dev only): log a single test near equinox (e.g., 2025‑09‑22 00:00 UTC) — verify:
   - Aries direction (RA≈0) points toward +Z stars; 
   - Sun apparent ecliptic long λ⊙≈180°, Earth’s λ≈0° and Earth vector points near +Z when mapped.

Why not touch WASM: compute_state returns standard heliocentric ecliptic axes (VSOP87). The sign inversion appears in TS mapping, not in WASM. WASM remains the source of truth and already matches astro-rust semantics.

Fallback plan: If we prefer to keep the current Earth mapping, alternatively rotate the star sphere basis (change `RotationYawPitchRoll(-RA, -Dec, 0)` to align RA=0 with +X) — but this is invasive and risks breaking many references; adjusting Earth mapping is the minimal, contained change.

Validation plan after fix:
- Visual: At present date, constellations should be behind Earth (not behind Sun) consistent with season.
- Numerical: Compare the angle between mapped Earth vector and stellar RA=0 direction near March/September equinoxes; expect sign-correct orientation opposite the Sun’s apparent RA.

### Open decisions / inputs needed
- Orion–SÜN formal target condition: provide precise definition (e.g., belt position angle vs local meridian at SÜН, or specific az/alt alignment). A minimal parametric target can be added first, then locked to canonical formula.
- Extended STATE vs helper exports: decide after step (1) perf review; initial approach is helpers to keep hot path minimal.

### Sanity checklist before coding next
- Run: `pnpm -w run dev:frontend-only` → validate scene loads, Safari console clean (no generateMipmap errors).
- `pnpm run prepare` to ensure hooks installed (if needed).
- Keep `astro-rust/` untouched. Any fmt/clippy run is per‑crate, not workspace‑wide.


