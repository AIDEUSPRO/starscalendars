# StarsCalendars – Canonical Context Bootstrap (2025-08-08)

This document is the single source of truth for agents. It resolves previous contradictions and pins the exact runtime/scene model and rules. Treat this file as the bootstrap when joining a new chat.

## Stack (major-only)
- Frontend: TypeScript 5.9, React 19, Vite 7, Babylon.js 8 (major; latest 8.x at build time)
- WASM core: Rust 1.91.1+, wasm-bindgen; local `astro-rust/` (READ-ONLY).
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
- Buffer layout (length = 11 f64):
  - Sun: x,y,z (geocentric)
  - Moon: x,y,z (geocentric)
  - Earth: x,y,z (heliocentric)
  - Solar zenith: [lon_east_rad, lat_rad]
- Extraction on scene:
  - Sun at (0,0,0) in scene (heliocentric visualization)
  - Earth uses Earth xyz from buffer (scale 1 AU = 700)
  - Moon position = Earth xyz + Moon geocentric offset
  - Zenith comes directly from the last 2 buffer values (radians)
- Z-convention: any required axis flips are handled once in the scene when assigning coordinates (single Z flip RH→LH). No flips in WASM bridge.

## Zenith marker and orientation (LOCKED — canonical)
- Create Earth pivot (`TransformNode`). Set `pivot.position` from Earth heliocentric position (scaled). Parent `earth` and `moonPivot` to `pivot`.
- Use zenith from `compute_state` buffer: `(lon_east_rad, lat_rad)` — exact radians. No degree conversions, no constants, no true anomaly tweaks.
- Place zenith marker in Earth-local spherical coordinates using ONLY WASM radians:
  - Let `phi = (π/2) - lat_rad`
  - Let `theta = (-lon_east_rad) + π`  // west-positive, place on correct surface side
  - `x = r * sin(phi) * cos(theta)`; `z = r * sin(phi) * sin(theta)`; `y = r * cos(phi)`; where `r = EarthDiameter * 0.5`
- Orient pivot so the line EarthCenter→Marker goes through scene origin (Sun):
  - `pivot.rotation.y = -((-lon_east_rad) + π)`
  - `pivot.rotation.z = lat_rad`
  - `pivot.rotation.x = lat_rad`
- Earth mesh keeps `rotation = (0,0,0)`; only pivot orients the hierarchy so Moon orbit follows tilt/azimuth.
- Single RH→LH Z-flip applies ONLY when assigning world positions from WASM; not used for marker math.
- This behavior is CANONICAL. Do not change without updating this file and running visual/ephemeris tests.

## Current achievements and next steps (2025‑08‑11)
- Achieved: sublunar point (lunar zenith) is computed from lunar RA/Dec + apparent sidereal time; matches external sources. Moon position uses the same vector chain; longitudes now align with the marker
- Next: move RA/Dec, AST, and sublunar φ/λ computation into `compute_state(jd)` so the scene does zero trigonometry (still exactly one call per frame). Also expose an Earth‑local unit vector for Earth→Moon to drive a visual tidal‑lock orientation (one side of the Moon facing Earth)

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

## Agent initialization prompt (paste into new chat)
```
Use docs/context-bootstrap.md as the single source of truth. Key rules:
 - Heliocentric scene (Sun@0,0,0), DIAMETER semantics (Earth=50, Moon=20, Sun=40), ENV_H=2, segments Earth/Clouds=300, Sun=15, Moon=25.
 - One compute_state(jd) per frame; zero-copy view; Z flip applied in scene (not in bridge).
 - Zenith in buffer (lonE, lat radians). Build pivot hierarchy and align marker to Sun.
 - Moon position and sublunar point must come from the same chain (RA/Dec + AST). No rotating lunar orbit with Earth pivot.
 - No manual mipmaps/anisotropy; Babylon GUI only; Canvas full-screen.
```


