# StarsCalendars – Canonical Context Bootstrap (2025-08-08)

This document is the single source of truth for agents. It resolves previous contradictions and pins the exact runtime/scene model and rules. Treat this file as the bootstrap when joining a new chat.

## Stack (major-only)
- Frontend: TypeScript 5.9, React 19, Vite 7, Babylon.js 8 (major; latest 8.x at build time)
- WASM core: Rust 1.92.0+, wasm-bindgen; local `astro-rust/` (READ-ONLY).
- Backend: Axum 0.8, Tokio 1, SQLX 0.8, Teloxide 0.13 (not required for scene)

## Scene Model (reference parity)
 - Coordinate system: Babylon default (left-handed) for rendering. Scientific data stays RH (WASM). Any axis flip (Z) is applied only at scene layer. Do not change the engine handedness.
- Heliocentric: Sun at (0,0,0). Earth uses heliocentric coordinates. Moon is geocentric relative to Earth.
- Scaling: 1 AU → 700 units (scaleAU=700). Artistic diameters: Earth=50, Moon=20, Sun=40; Atmosphere shell height (ENV_H)=2.
- Mesh segments: Earth=300, Clouds=300, Sun=15, Moon=25.
- DIAMETER semantics: configured numbers are DIAMETERS passed to Babylon mesh `diameter` (not radii). When radius is needed, use `diameter*0.5`.
- Camera: ArcRotateCamera, FOV=1.5, target Earth. minZ≈0.1, maxZ≈200000. Zoom: [earthRadius*1.1, earthRadius*50].
- Skybox: `new CubeTexture('/textures/universe/universe', scene)` bound to a box size 10000. No manual mipmap toggling.
- Sun FX: FireProceduralTexture size=128 → emissiveTexture; GodRays samples=100; tuned exposure/decay/weight/density as in ref.
- GUI: Babylon GUI only; show current time and quantum date. Stats overlay `#stats` for FPS.

## Quick Start (development)

```bash
# 1) Установка зависимостей (workspace)
cd /Volumes/WXW/R/_ai_/starscalendars pnpm -w install

# 2) Запуск фронтенда (пересобирает WASM и стартует Vite dev)
cd /Volumes/WXW/R/_ai_/starscalendars/frontend && pnpm -w run dev:frontend-only
```

Notes:
- WASM обертка использует ТОЛЬКО локальную `astro-rust/` (READ-ONLY); любые правки запрещены.
- Координаты RH из WASM; единичный RH→LH Z‑flip применяется ТОЛЬКО в слое сцены при присвоении позиций.

## WASM contract and usage
- Exactly one `compute_state(jd: f64) -> *const f64` per frame. Zero-copy Float64Array view on memory.
- Buffer layout (length = 27 f64) — CANONICAL (append-only):
  - `0..2`  Sun zeros (x,y,z = 0) by design (Sun fixed at origin; skip per-frame solar xyz math)
  - `3`     Moon distance AU (geocentric)
  - `4`     Earth heliocentric RA (rad)
  - `5`     Earth heliocentric Dec (rad)
  - `6`     Earth–Sun distance AU
  - `7`     Solar zenith longitude (rad, east-positive)
  - `8`     Solar zenith latitude (rad, north-positive)
  - `9`     Sublunar latitude (rad)
  - `10`    Sublunar longitude (rad, east-positive)
  - `11..13` Earth-local unit vector toward Moon [x,y,z]
  - `14`    Apparent sidereal time (rad)
  - `15`    Sun apparent ecliptic longitude (rad; FK5 + aberration + nutation)
  - `16`    Moon ecliptic longitude (rad; with nutation)
  - `17`    Moon ecliptic latitude (rad)
  - `18`    Moon illumination fraction [0..1]
  - `19`    Moon–Sun elongation (rad; [0, 2π))
  - `20`    Sun zodiac tropical index 0..11
  - `21`    Moon zodiac tropical index 0..11
  - `22`    Sun zodiac sidereal index (MVP=J2000) 0..11
  - `23`    Moon zodiac sidereal index (MVP=J2000) 0..11
  - `24`    Moon true ascending node longitude (rad; [0, 2π))
  - `25`    Moon mean perigee longitude (rad; [0, 2π))
  - `26`    Moon phase8 id 0..7
- Extraction on scene:
  - Sun stays at (0,0,0) (heliocentric visualization; slots 0..2 are unused)
  - Earth position from RA/Dec/dist (slots 4..6) → spherical→cartesian, then RH→LH single Z flip
  - Earth orientation from zenith lon/lat (slots 7..8) via pivot quaternion (see tz.md “Earth pivot orientation — canonical”)
  - Moon position from Earth-local unit vector (11..13) rotated by pivot quaternion and scaled by Moon distance (3)
  - Sublunar marker from (9..10) using the same Earth-local spherical mapping as zenith marker
- Z-convention: any required axis flips are handled once in the scene when assigning coordinates (single Z flip RH→LH). No flips in WASM bridge.

## Zenith marker and orientation (LOCKED — canonical)
- Create Earth pivot (`TransformNode`). Set `pivot.position` from Earth heliocentric position (scaled). Parent `earth` and `moonPivot` to `pivot`.
- Use zenith from `compute_state` buffer: `(lon_east_rad, lat_rad)` — exact radians. No degree conversions, no constants, no true anomaly tweaks.
- Place zenith marker in Earth-local spherical coordinates using ONLY WASM radians:
  - Let `phi = (π/2) - lat_rad`
  - Let `theta = (-lon_east_rad) + π`  // west-positive, place on correct surface side
  - `x = r * sin(phi) * cos(theta)`; `z = r * sin(phi) * sin(theta)`; `y = r * cos(phi)`; where `r = EarthDiameter * 0.5`
- Orient pivot so the line EarthCenter→zenithMarker points to the Sun (scene origin). **Implementation is quaternion-based** in `frontend/src/scene/BabylonScene.tsx` to avoid Euler edge cases:
  - Compute `zenithLocalVector` from `(lon_east_rad, lat_rad)` using the spherical mapping above
  - Compute `targetDirVector = normalize(SunWorldPos - EarthWorldPos)` (Sun at origin → `-EarthWorldPos`)
  - Compute `q_align` that rotates `zenithLocalVector → targetDirVector` (handle parallel/opposite vectors)
  - Compute a “roll” correction around `targetDirVector` to keep “local North” aligned with projected `worldUp`
  - Final quaternion = `q_roll * q_align`, normalized; assign to `earthPivot.rotationQuaternion`; force `earthPivot.rotation = (0,0,0)`
- Earth mesh keeps `rotation = (0,0,0)`; only pivot orients the hierarchy so Moon orbit follows tilt/azimuth.
- Single RH→LH Z-flip applies ONLY when assigning world positions from WASM; not used for marker math.
- This behavior is CANONICAL. Do not change without updating this file and running visual/ephemeris tests.

## Camera presets (Earth ↔ Moon) — CANONICAL
- Camera is `ArcRotateCamera`. Scene supports 2 target modes: `cameraTarget = 'earth' | 'moon'`.
- Earth preset (`🌍` and on scene start):
  - Approximate user longitude from timezone: `lon_deg = -tzOffsetMinutes / 4`
  - Compute Earth-surface local point via `latLonToLocalXYZ(latDeg, lonDeg, earthRadius)`
  - Camera pos = EarthWorldPos + localPoint + normal(localPoint)*offset
  - **Two-phase apply** (prevents ArcRotate internal alpha/beta/radius drift when switching modes):
    - Phase 1: `detachControl(); lockedTarget=null; setTarget(earthPos); setPosition(cameraPos); scene.render()`
    - Phase 2: `lockedTarget=earthMesh; attachControl(canvas,true); scene.render()`
  - Reset limits (important after Moon mode): alpha/beta free, radius clamped for Earth view.
- Moon preset (`🌙`):
  - Use **world** positions: `moonWorldPos = moonMesh.getAbsolutePosition()`, `earthWorldPos = earthPivot.position`
  - **Two-phase apply**:
    - Phase 1: `detachControl(); lockedTarget=null; setTarget(moonWorldPos); setPosition(earthWorldPos); scene.render()`
    - Phase 2: `lockedTarget=moonMesh; attachControl(canvas,true); scene.render()`
  - Then lock rotation by clamping `alpha/beta` to current values; allow zoom by setting `lowerRadiusLimit/upperRadiusLimit` from the now-updated `cam.radius`.

## Current achievements and next steps (2025‑08‑11)
- Achieved:
  - `compute_state(jd)` is the single hot-path call and returns STATE[27] (append-only: base [0..14] for scene geometry + appended [15..26] for zodiac/events UI).
  - Sublunar point (lunar zenith) is derived from lunar RA/Dec + apparent sidereal time and matches external sources; Moon position uses the same chain.
  - Camera presets Earth↔Moon are implemented with a robust two-phase ArcRotate apply (see section above).
- Next (current active cycle):
  - Add zodiac (tropical/sidereal) and lunar events (phases/nodes/apsides/eclipses/voc) based strictly on astro-rust outputs (events may be derived classifiers/search).
  - Add a Moon-view info panel (Babylon GUI) near the Moon and a “РАСШИФРОВКА ДНЯ в @elioncalendar” hint inside scene top on quantum time.

## Textures
- Do not force `noMipmap` or `anisotropicFilteringLevel`. Use Babylon defaults.
- Earth material: custom day/night shader with `earth-diffuse.jpg` and `earth-night-o2.png` and height map `earth-height.png` applied once mesh ready.
- Clouds: custom shader, texture `earth-c.jpg`, shell diameter = Earth diameter + 2, rotation.z = π, parent=earth.

## Quantum date (NT)
- Реализация перенесена в WASM: `get_quantum_time_components(epoch_ms, tz_offset_min)` возвращает `[d_in_decade, decade, year]`
- Семантика идентична прежней JS (константы/особые дни), но вычисление централизовано и кэшируемо
- Обновление текста NT — раз в минуту в idle, без участия рендер-цикла

## Quality and rules
- One render loop, no extra timers. No mock data. No manual performance hacks that conflict with reference parity.
- Major-only versions in package manifests and Cargo.toml (e.g., `"@babylonjs/core": "8"`, `axum = "0.8"`).
- `astro-rust/` is read-only.


## What is implemented now (BabylonScene.tsx)
- Reference mesh sizes/segments set. Fire=128, GodRays=100. Skybox via base path. GUI matches ref. No manual mipmaps.
- Zenith marker placement is LOCKED per rules above (pure WASM radians, no tweaks). Pivot orientation aligns marker to the Sun. Moon follows pivot tilt.

## Stats overlay (#stats)
- There is exactly one HTML overlay div: `#stats` (FPS). No other overlays are allowed.
- If additional UI is needed (e.g. “РАСШИФРОВКА ДНЯ в @elioncalendar”), it must be rendered **inside** scene top on quantum time.

## Agent initialization prompt (paste into new chat)
```
Use docs/context-bootstrap.md as the single source of truth. Key rules:
 - Heliocentric scene (Sun@0,0,0), DIAMETER semantics (Earth=50, Moon=20, Sun=40), ENV_H=2, segments Earth/Clouds=300, Sun=15, Moon=25.
 - One compute_state(jd) per frame; zero-copy view; Z flip applied in scene (not in bridge).
 - Zenith in buffer (lonE, lat radians). Build pivot hierarchy and align marker to Sun.
 - Moon position and sublunar point must come from the same chain (RA/Dec + AST). No rotating lunar orbit with Earth pivot.
 - No manual mipmaps/anisotropy; Babylon GUI only; Canvas full-screen.
```


