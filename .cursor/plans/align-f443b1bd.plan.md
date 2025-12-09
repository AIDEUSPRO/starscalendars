<!-- f443b1bd-d410-4535-a102-6f7c7f2db0eb 2e37a9ef-f1f7-4aca-8780-d953fc182f3e -->
# Earth–Star Alignment Fix

1. **Audit Current Mapping**  

- Verify `compute_state` output (heliocentric ecliptic XYZ) and how BabylonScene maps `(ex, ey, ez)` to scene axes.  
- Confirm starfield uses RA/Dec rotation so we know the exact reference frame mismatch.

2. **Extend WASM State**  

- In `wasm-astro/src/lib.rs`, augment `compute_state` (or add a companion export) to compute Earth’s geocentric equatorial coordinates (RA, Dec, distance) using astro-rust (`planet::geocent_apprnt_ecl_coords`, `coords::asc_frm_ecl`, `coords::dec_frm_ecl`).  
- Keep existing XYZ slots for legacy use but append RA/Dec + radius for zero-copy access.

3. **Update Frontend Integration**  

- In `frontend/src/scene/BabylonScene.tsx`, switch Earth positioning to use RA/Dec from the new STATE slots: build the same rotation matrix as for stars (yaw/pitch from RA/Dec) and place Earth at correct distance.  
- Apply the same RA/Dec-based transform when drawing the orbit polyline and perihelion/aphelion markers so all visuals share one reference frame.

4. **Validation & Debugging**  

- Log RA/Dec + Babylon vectors for a known date (e.g., 2025-11-16) and compare with external ephemeris/Orión position to confirm the hemisphere matches expectations.  
- Re-run visual check ensuring Earth appears on the Orion-side before perihelion.  
- Update docs (e.g., `docs/wasm-astro-api-checklist.md`) to document new STATE layout.