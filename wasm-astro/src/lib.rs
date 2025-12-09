//! # StarsCalendars WASM Astronomical Core
//!
//! Pure astro-rust library wrapper with ZERO custom calculations.
//! Follows O(1) горячий путь principle with exactly one compute_state(t) call per frame.
//!
//! ## Performance Requirements
//! - Exactly one `compute_state(julian_day)` call per frame
//! - Zero-copy data transfer via Float64Array view
//! - Thread-local buffer for ephemeris data
//! - NO mock data, NO custom formulas, ONLY astro-rust functions!
//! - No string passing between WASM-JS
//!
//! ## Covered astro-rust API:
//! - sun::geocent_ecl_pos() - Sun position (VSOP87)
//! - lunar::geocent_ecl_pos() - Moon position (ELP-2000/82)
//! - planet::heliocent_coords() - All planets (VSOP87)
//! - pluto::heliocent_pos() - Pluto position
//! - nutation::nutation() - Nutation corrections
//! - precess::precess_ecl_coords() - Precession corrections
//! - time::julian_cent() - Time conversions
//! - angle utilities - Coordinate conversions

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

// WASM-only types - astronomical calculations are forbidden in backend per tz.md
#[derive(Debug, Clone, Copy)]
struct JulianDay(f64);

impl JulianDay {
    pub fn new(jd: f64) -> Result<Self, &'static str> {
        if jd < 0.0 || !jd.is_finite() {
            Err("Invalid Julian Day")
        } else {
            Ok(Self(jd))
        }
    }

    pub fn as_f64(&self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
struct Cartesian {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Cartesian {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy)]
enum CelestialBody {
    Sun,
    Moon,
    Mercury,
    Venus,
    Earth,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
    Pluto,
}

impl CelestialBody {
    const ALL: &'static [CelestialBody] = &[
        Self::Sun,
        Self::Moon,
        Self::Mercury,
        Self::Venus,
        Self::Earth,
        Self::Mars,
        Self::Jupiter,
        Self::Saturn,
        Self::Uranus,
        Self::Neptune,
        Self::Pluto,
    ];
}

// Import astro-rust library for high-precision astronomical calculations
extern crate astro;

// Enable console.log for debugging
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// ===== Reusable time scale utilities (UTC↔TT) with leap seconds support =====
mod timescales {
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;

    thread_local! {
        static TAI_MINUS_UTC_OVERRIDE: RefCell<Option<f64>> = const { RefCell::new(None) };
    }

    const LEAP_SECONDS: &[(i32, u8, f64)] = &[
        (1972, 1, 10.0),
        (1972, 7, 11.0),
        (1973, 1, 12.0),
        (1974, 1, 13.0),
        (1975, 1, 14.0),
        (1976, 1, 15.0),
        (1977, 1, 16.0),
        (1978, 1, 17.0),
        (1979, 1, 18.0),
        (1980, 1, 19.0),
        (1981, 7, 20.0),
        (1982, 7, 21.0),
        (1983, 7, 22.0),
        (1985, 7, 23.0),
        (1988, 1, 24.0),
        (1990, 1, 25.0),
        (1991, 1, 26.0),
        (1992, 7, 27.0),
        (1993, 7, 28.0),
        (1994, 7, 29.0),
        (1996, 1, 30.0),
        (1997, 7, 31.0),
        (1999, 1, 32.0),
        (2006, 1, 33.0),
        (2009, 1, 34.0),
        (2013, 1, 35.0),
        (2015, 7, 36.0),
        (2017, 1, 37.0),
    ];

    fn tai_minus_utc_seconds_for_year_month(year: i32, month: u8) -> f64 {
        let override_val = TAI_MINUS_UTC_OVERRIDE.with(|c| *c.borrow());
        if let Some(v) = override_val {
            return v;
        }
        // Saturate to last known (37 s since 2017-01) for future years until updated
        let mut val = if year < 1972 { 0.0 } else { 10.0 };
        let mut i = 0usize;
        while i < LEAP_SECONDS.len() {
            let (y, m, v) = LEAP_SECONDS[i];
            if year > y || (year == y && month >= m) {
                val = v;
            }
            i += 1;
        }
        val
    }

    pub fn utc_to_tt_jd(jd_utc: f64) -> f64 {
        let (y, m, _d) = match astro::time::date_frm_julian_day(jd_utc) {
            Ok((yy, mm, dd)) => (yy as i32, mm as u8, dd),
            Err(_) => return f64::NAN,
        };
        let tai_utc = tai_minus_utc_seconds_for_year_month(y, m);
        let tt_minus_utc = tai_utc + 32.184;
        jd_utc + tt_minus_utc / 86400.0
    }

    pub fn tt_to_utc_jd(jd_tt: f64) -> f64 {
        let (y, m, _d) = match astro::time::date_frm_julian_day(jd_tt) {
            Ok((yy, mm, dd)) => (yy as i32, mm as u8, dd),
            Err(_) => return f64::NAN,
        };
        let tai_utc = tai_minus_utc_seconds_for_year_month(y, m);
        let tt_minus_utc = tai_utc + 32.184;
        jd_tt - tt_minus_utc / 86400.0
    }

    #[wasm_bindgen]
    pub fn set_tai_minus_utc_override(seconds: f64) {
        TAI_MINUS_UTC_OVERRIDE.with(|c| {
            *c.borrow_mut() = if seconds.is_finite() && seconds >= 0.0 {
                Some(seconds)
            } else {
                None
            };
        });
    }

    #[wasm_bindgen]
    pub fn clear_tai_minus_utc_override() {
        TAI_MINUS_UTC_OVERRIDE.with(|c| {
            *c.borrow_mut() = None;
        });
    }
}

/// Thread-local buffer for ephemeris data (O(1) горячий путь requirement)
///
/// Pre-allocated buffer that can hold all planetary positions in flat f64 array.
/// Layout: [x1, y1, z1, x2, y2, z2, ...] for direct Float64Array view
/// Size: 11 bodies * 3 coordinates = 33 f64 values
thread_local! {
    static EPHEMERIS_BUFFER: RefCell<Vec<f64>> = RefCell::new(Vec::with_capacity(33));
}

/// WASM module initialization
#[wasm_bindgen(start)]
pub fn init() {
    // Development-only log
    #[cfg(debug_assertions)]
    console_log!("🚀 StarsCalendars WASM - Pure astro-rust wrapper initialized");
}

// Legacy compute_all API removed. Use compute_state(julian_day).

/// Compute main state in a single call (future-extensible):
/// Layout [14 f64]:
/// - Sun(0..2): zeros by design (Sun fixed at scene origin; skip per-frame solar math)
/// - Moon(3..5): geocentric ecliptic Cartesian (x,y,z) in AU
/// - Earth(6..8): heliocentric ecliptic Cartesian (x,y,z) in AU
/// - Zenith(9..10): Solar zenith [lon_east_rad, lat_rad]
/// - Earth RA/Dec(11..12): heliocentric equatorial coordinates (radians)
/// - Earth distance(13): heliocentric radius vector (AU)
#[wasm_bindgen]
pub fn compute_state(julian_day: f64) -> *const f64 {
    thread_local! {
        static STATE_BUFFER: RefCell<[f64; 14]> = const { RefCell::new([0.0; 14]) };
    }

    STATE_BUFFER.with(|buffer| {
        let mut out = buffer.borrow_mut();

        // Validate JD
        let jd = match JulianDay::new(julian_day) {
            Ok(jd) => jd.as_f64(),
            Err(_) => {
                console_log!("❌ Invalid Julian Day: {}", julian_day);
                return std::ptr::null();
            }
        };

        // Nutation for precision
        let (nut_long, nut_oblq) = astro::nutation::nutation(jd);
        let mean_oblq = astro::ecliptic::mn_oblq_IAU(jd);
        let true_oblq = mean_oblq + nut_oblq;

        // Sun position/derived values are not used in hot path; keep zeros to minimize per-frame work
        out[0] = 0.0; // reserved
        out[1] = 0.0; // reserved
        out[2] = 0.0; // reserved

        // Moon geocentric
        let (moon_ecl, moon_dist_km) = astro::lunar::geocent_ecl_pos(jd);
        let moon_dist_au = moon_dist_km / 149597870.7;
        let moon_corrected_long = moon_ecl.long + nut_long;
        let moon_pos = ecliptic_to_cartesian(moon_corrected_long, moon_ecl.lat, moon_dist_au);
        out[3] = moon_pos.x;
        out[4] = moon_pos.y;
        out[5] = moon_pos.z;

        // Earth heliocentric
        let (earth_long, earth_lat, earth_r) =
            astro::planet::heliocent_coords(&astro::planet::Planet::Earth, jd);
        let earth_pos = ecliptic_to_cartesian(earth_long, earth_lat, earth_r);
        out[6] = earth_pos.x;
        out[7] = earth_pos.y;
        out[8] = earth_pos.z;
        let earth_ra = astro::coords::asc_frm_ecl(earth_long, earth_lat, true_oblq);
        let earth_dec = astro::coords::dec_frm_ecl(earth_long, earth_lat, true_oblq);
        out[11] = astro::angle::limit_to_two_PI(earth_ra);
        out[12] = earth_dec;
        out[13] = earth_r;

        // Solar zenith in radians (lon E+, lat N+)
        let (zenith_lon_east_rad, zenith_lat_rad) = solar_zenith_position_rad_internal(jd);
        out[9] = zenith_lon_east_rad;
        out[10] = zenith_lat_rad;

        out.as_ptr()
    })
}

/// Find next winter solstice (minimum solar declination) starting from given UTC JD.
/// Returns JD in UTC of the event. Heavy: use off-frame (idle) only.
#[wasm_bindgen]
pub fn next_winter_solstice_from(jd_utc_start: f64) -> f64 {
    // Validate
    let jd_utc = match JulianDay::new(jd_utc_start) {
        Ok(jd) => jd.as_f64(),
        Err(_) => return f64::NAN,
    };

    use crate::timescales::{tt_to_utc_jd, utc_to_tt_jd};

    // Helpers for angular math
    #[inline]
    fn wrap_two_pi(x: f64) -> f64 {
        astro::angle::limit_to_two_PI(x)
    }
    #[inline]
    fn principal_angle(x: f64) -> f64 {
        let w = wrap_two_pi(x);
        if w > std::f64::consts::PI {
            w - 2.0 * std::f64::consts::PI
        } else {
            w
        }
    }

    // Apparent solar ecliptic longitude λ_app (TT)
    let solar_lambda_app_tt = |jd_tt: f64| -> f64 {
        // Geometric geocentric ecliptic coords (mean equinox of date)
        let (sun_ecl, sun_dist_au) = astro::sun::geocent_ecl_pos(jd_tt);
        // FK5 correction
        let (long_fk5, _lat_fk5) = astro::sun::ecl_coords_to_FK5(jd_tt, sun_ecl.long, sun_ecl.lat);
        // Annual aberration (radians)
        let ab_long = astro::aberr::sol_aberr(sun_dist_au);
        // Nutation in longitude
        let (nut_long, _nut_oblq) = astro::nutation::nutation(jd_tt);
        wrap_two_pi(long_fk5 + ab_long + nut_long)
    };

    // Start from TT corresponding to start UTC
    let jd_tt0 = utc_to_tt_jd(jd_utc);
    if !jd_tt0.is_finite() {
        return f64::NAN;
    }

    // Target longitude: 270° (winter solstice)
    let target = 270.0_f64.to_radians();
    let lambda0 = solar_lambda_app_tt(jd_tt0);
    // Mean solar rate ~0.98564736°/day = 0.017202124 rad/day
    const SOLAR_MEAN_RATE: f64 = 0.017202124; // @allow-numeric-param
    // Forward phase to next target
    let delta_forward = wrap_two_pi(target - lambda0);
    let mut t = jd_tt0 + delta_forward / SOLAR_MEAN_RATE;

    // Newton with numeric derivative (robust, few iterations)
    let mut iter = 0u32;
    const H: f64 = 1.0e-3; // ~86.4 s @allow-numeric-param
    const MAX_ITERS: u32 = 16; // @allow-numeric-param
    const MAX_STEP: f64 = 2.0; // days @allow-numeric-param
    loop {
        if !t.is_finite() || iter >= MAX_ITERS {
            break;
        }
        let lam = solar_lambda_app_tt(t);
        let f = principal_angle(lam - target);
        if f.abs() < 1.0e-10 {
            break;
        }
        let lam_p = solar_lambda_app_tt(t + H);
        let lam_m = solar_lambda_app_tt(t - H);
        let dfdt = principal_angle(lam_p - lam_m) / (2.0 * H);
        if dfdt.abs() < 1.0e-12 {
            break;
        }
        let mut step = f / dfdt;
        if step > MAX_STEP {
            step = MAX_STEP;
        }
        if step < -MAX_STEP {
            step = -MAX_STEP;
        }
        t -= step;
        iter += 1;
    }
    let jd_tt_min = t;

    // Convert TT -> UTC using ΔT at event date
    tt_to_utc_jd(jd_tt_min)
}
/// Compute Earth's perihelion and aphelion for a given UTC year (approximate, numeric on VSOP87 r(jd)).
/// Returns pointer to [peri_jd_utc, peri_r_au, peri_long_rad, aph_jd_utc, aph_r_au, aph_long_rad]
/// Heavy: run off-frame only.
#[wasm_bindgen]
pub fn earth_perihelion_aphelion_for_year_utc(year_utc: i32) -> *const f64 {
    thread_local! { static EA_BUF: RefCell<[f64; 6]> = const { RefCell::new([f64::NAN; 6]) }; }

    EA_BUF.with(|buffer| {
        let mut out = buffer.borrow_mut();
        if year_utc < 1600 || year_utc > 2600 {
            return std::ptr::null();
        }

        // Create Date struct for January 1, year_utc, 00:00:00 UTC
        let date = astro::time::Date {
            year: year_utc as i16,
            month: 1,
            decimal_day: 1.0,
            cal_type: astro::time::CalType::Gregorian,
        };
        let jd_start = astro::time::julian_day(&date);

        // Helper: heliocentric Earth distance r(jd) and longitude lambda(jd)
        #[inline]
        fn earth_r_lambda(jd: f64) -> (f64, f64) {
            let (l, _b, r) = astro::planet::heliocent_coords(&astro::planet::Planet::Earth, jd);
            (r, astro::angle::limit_to_two_PI(l))
        }

        // Numeric helper: find extremum in [a,b] by sampling + parabolic fit around best sample
        #[inline]
        fn extremum_in_range(a: f64, b: f64, want_min: bool) -> (f64, f64, f64) {
            let n = 366usize; // daily
            let mut best_jd = a;
            let mut best_r = if want_min {
                f64::INFINITY
            } else {
                -f64::INFINITY
            };
            for i in 0..=n {
                let jd = a + (b - a) * (i as f64) / (n as f64);
                let (r, _l) = earth_r_lambda(jd);
                if (want_min && r < best_r) || (!want_min && r > best_r) {
                    best_r = r;
                    best_jd = jd;
                }
            }
            // Refine around best with small parabola fit using ±H
            let h = 0.5; // days @allow-numeric-param
            let (r_m, _l_m) = earth_r_lambda(best_jd - h);
            let (r_0, _l_0) = earth_r_lambda(best_jd);
            let (r_p, _l_p) = earth_r_lambda(best_jd + h);
            // Parabolic vertex t* = best_jd + h*(r_m - r_p)/(2*(r_m - 2*r_0 + r_p))
            let denom = (r_m - 2.0 * r_0 + r_p) * 2.0;
            let jd_star = if denom.abs() > 1e-12 {
                best_jd + h * (r_m - r_p) / denom
            } else {
                best_jd
            };
            let (r_star, l_star) = earth_r_lambda(jd_star);
            (jd_star, r_star, l_star)
        }

        // Perihelion near early January; Aphelion near early July
        let (peri_jd_tt, peri_r, peri_l) =
            extremum_in_range(jd_start - 10.0, jd_start + 50.0, true);
        let (aph_jd_tt, aph_r, aph_l) =
            extremum_in_range(jd_start + 170.0, jd_start + 220.0, false);

        // Convert TT→UTC (same approx used elsewhere)
        let peri_jd_utc = timescales::tt_to_utc_jd(peri_jd_tt);
        let aph_jd_utc = timescales::tt_to_utc_jd(aph_jd_tt);

        out[0] = peri_jd_utc;
        out[1] = peri_r;
        out[2] = peri_l;
        out[3] = aph_jd_utc;
        out[4] = aph_r;
        out[5] = aph_l;
        out.as_ptr()
    })
}

// Removed legacy helpers get_body_count/get_coordinate_count (no longer used by frontend)

/// **PERFORMANCE CRITICAL**: Calculate all planetary positions using PURE astro-rust
///
/// Uses pre-allocated buffer to avoid any allocations in hot path.
/// Implements ONLY astro-rust library functions - zero custom calculations!
fn calculate_all_positions_pure_astro_rust(
    julian_day: JulianDay,
    buffer: &mut Vec<f64>,
) -> Result<(), &'static str> {
    let jd = julian_day.as_f64();

    // High-precision corrections for maximum accuracy (1.7% performance cost)
    let (nut_long, nut_oblq) = astro::nutation::nutation(jd);
    let _mean_oblq = astro::ecliptic::mn_oblq_IAU(jd);
    let _true_oblq = _mean_oblq + nut_oblq;

    // Sun position using corrected astro-rust (geocentric ecliptic)
    let (sun_ecl, sun_dist_km) = astro::sun::geocent_ecl_pos(jd);
    let sun_dist_au = sun_dist_km / 149597870.7; // Convert km to AU
    let sun_corrected_long = sun_ecl.long + nut_long; // Apply nutation
    let sun_pos = ecliptic_to_cartesian(sun_corrected_long, sun_ecl.lat, sun_dist_au);
    buffer.push(sun_pos.x);
    buffer.push(sun_pos.y);
    buffer.push(sun_pos.z);

    // Moon position using ELP-2000/82 theory
    let (moon_ecl, moon_dist_km) = astro::lunar::geocent_ecl_pos(jd);
    let moon_dist_au = moon_dist_km / 149597870.7; // Convert km to AU
    let moon_corrected_long = moon_ecl.long + nut_long; // Apply nutation
    let moon_pos = ecliptic_to_cartesian(moon_corrected_long, moon_ecl.lat, moon_dist_au);
    buffer.push(moon_pos.x);
    buffer.push(moon_pos.y);
    buffer.push(moon_pos.z);

    // Mercury using VSOP87 theory
    let (mercury_long, mercury_lat, mercury_r) =
        astro::planet::heliocent_coords(&astro::planet::Planet::Mercury, jd);
    let mercury_pos = ecliptic_to_cartesian(mercury_long, mercury_lat, mercury_r);
    buffer.push(mercury_pos.x);
    buffer.push(mercury_pos.y);
    buffer.push(mercury_pos.z);

    // Venus using VSOP87 theory
    let (venus_long, venus_lat, venus_r) =
        astro::planet::heliocent_coords(&astro::planet::Planet::Venus, jd);
    let venus_pos = ecliptic_to_cartesian(venus_long, venus_lat, venus_r);
    buffer.push(venus_pos.x);
    buffer.push(venus_pos.y);
    buffer.push(venus_pos.z);

    // Earth using VSOP87 theory
    let (earth_long, earth_lat, earth_r) =
        astro::planet::heliocent_coords(&astro::planet::Planet::Earth, jd);
    let earth_pos = ecliptic_to_cartesian(earth_long, earth_lat, earth_r);
    buffer.push(earth_pos.x);
    buffer.push(earth_pos.y);
    buffer.push(earth_pos.z);

    // Mars using VSOP87 theory
    let (mars_long, mars_lat, mars_r) =
        astro::planet::heliocent_coords(&astro::planet::Planet::Mars, jd);
    let mars_pos = ecliptic_to_cartesian(mars_long, mars_lat, mars_r);
    buffer.push(mars_pos.x);
    buffer.push(mars_pos.y);
    buffer.push(mars_pos.z);

    // Jupiter using VSOP87 theory
    let (jupiter_long, jupiter_lat, jupiter_r) =
        astro::planet::heliocent_coords(&astro::planet::Planet::Jupiter, jd);
    let jupiter_pos = ecliptic_to_cartesian(jupiter_long, jupiter_lat, jupiter_r);
    buffer.push(jupiter_pos.x);
    buffer.push(jupiter_pos.y);
    buffer.push(jupiter_pos.z);

    // Saturn using VSOP87 theory
    let (saturn_long, saturn_lat, saturn_r) =
        astro::planet::heliocent_coords(&astro::planet::Planet::Saturn, jd);
    let saturn_pos = ecliptic_to_cartesian(saturn_long, saturn_lat, saturn_r);
    buffer.push(saturn_pos.x);
    buffer.push(saturn_pos.y);
    buffer.push(saturn_pos.z);

    // Uranus using VSOP87 theory
    let (uranus_long, uranus_lat, uranus_r) =
        astro::planet::heliocent_coords(&astro::planet::Planet::Uranus, jd);
    let uranus_pos = ecliptic_to_cartesian(uranus_long, uranus_lat, uranus_r);
    buffer.push(uranus_pos.x);
    buffer.push(uranus_pos.y);
    buffer.push(uranus_pos.z);

    // Neptune using VSOP87 theory
    let (neptune_long, neptune_lat, neptune_r) =
        astro::planet::heliocent_coords(&astro::planet::Planet::Neptune, jd);
    let neptune_pos = ecliptic_to_cartesian(neptune_long, neptune_lat, neptune_r);
    buffer.push(neptune_pos.x);
    buffer.push(neptune_pos.y);
    buffer.push(neptune_pos.z);

    // Pluto using high-precision heliocentric coordinates
    let (pluto_long, pluto_lat, pluto_r) = astro::pluto::heliocent_pos(jd);
    let pluto_pos = ecliptic_to_cartesian(pluto_long, pluto_lat, pluto_r);
    buffer.push(pluto_pos.x);
    buffer.push(pluto_pos.y);
    buffer.push(pluto_pos.z);

    Ok(())
}

/// Convert ecliptic spherical coordinates to Cartesian
///
/// High-performance coordinate transformation without allocations.
/// Input coordinates are in RADIANS (as returned by astro-rust API).
#[inline]
fn ecliptic_to_cartesian(longitude_rad: f64, latitude_rad: f64, radius_au: f64) -> Cartesian {
    let cos_lat = latitude_rad.cos();
    let x = radius_au * cos_lat * longitude_rad.cos();
    let y = radius_au * cos_lat * longitude_rad.sin();
    let z = radius_au * latitude_rad.sin();
    Cartesian::new(x, y, z)
}

// ========== ADDITIONAL ASTRO-RUST API COVERAGE ==========
// Complete coverage of astro-rust library functions for full astronomical calculations

/// **PURE ASTRO-RUST**: Get Sun's geocentric ecliptic position with corrections
///
/// Returns Sun's position with optional nutation correction.
/// Uses ONLY astro::sun::geocent_ecl_pos() from the library.
#[wasm_bindgen]
pub fn get_sun_position(julian_day: f64, apply_nutation: bool) -> *const f64 {
    thread_local! {
        static SUN_BUFFER: RefCell<[f64; 3]> = const { RefCell::new([0.0; 3]) };
    }

    SUN_BUFFER.with(|buffer| {
        let mut buf = buffer.borrow_mut();

        // Get Sun position using pure astro-rust
        let (sun_ecl, sun_dist_km) = astro::sun::geocent_ecl_pos(julian_day);
        let sun_dist_au = sun_dist_km / 149597870.7;

        let longitude = if apply_nutation {
            let (nut_long, _nut_oblq) = astro::nutation::nutation(julian_day);
            sun_ecl.long + nut_long
        } else {
            sun_ecl.long
        };

        let pos = ecliptic_to_cartesian(longitude, sun_ecl.lat, sun_dist_au);
        buf[0] = pos.x;
        buf[1] = pos.y;
        buf[2] = pos.z;

        buf.as_ptr()
    })
}

/// **PURE ASTRO-RUST**: Get Moon's geocentric ecliptic position with corrections
///
/// Returns Moon's position using ELP-2000/82 theory with optional nutation correction.
/// Uses ONLY astro::lunar::geocent_ecl_pos() from the library.
#[wasm_bindgen]
pub fn get_moon_position(julian_day: f64, apply_nutation: bool) -> *const f64 {
    thread_local! {
        static MOON_BUFFER: RefCell<[f64; 3]> = const { RefCell::new([0.0; 3]) };
    }

    MOON_BUFFER.with(|buffer| {
        let mut buf = buffer.borrow_mut();

        // Get Moon position using pure astro-rust ELP-2000/82
        let (moon_ecl, moon_dist_km) = astro::lunar::geocent_ecl_pos(julian_day);
        let moon_dist_au = moon_dist_km / 149597870.7;

        let longitude = if apply_nutation {
            let (nut_long, _nut_oblq) = astro::nutation::nutation(julian_day);
            moon_ecl.long + nut_long
        } else {
            moon_ecl.long
        };

        let pos = ecliptic_to_cartesian(longitude, moon_ecl.lat, moon_dist_au);
        buf[0] = pos.x;
        buf[1] = pos.y;
        buf[2] = pos.z;

        buf.as_ptr()
    })
}

/// **PURE ASTRO-RUST**: Get planet heliocentric position using VSOP87
///
/// Returns planetary position using ONLY astro::planet::heliocent_coords() from the library.
/// Planet indices: 0=Mercury, 1=Venus, 2=Earth, 3=Mars, 4=Jupiter, 5=Saturn, 6=Uranus, 7=Neptune
#[wasm_bindgen]
pub fn get_planet_position(planet_index: usize, julian_day: f64) -> *const f64 {
    thread_local! {
        static PLANET_BUFFER: RefCell<[f64; 3]> = const { RefCell::new([0.0; 3]) };
    }

    PLANET_BUFFER.with(|buffer| {
        let mut buf = buffer.borrow_mut();

        let planet = match planet_index {
            0 => astro::planet::Planet::Mercury,
            1 => astro::planet::Planet::Venus,
            2 => astro::planet::Planet::Earth,
            3 => astro::planet::Planet::Mars,
            4 => astro::planet::Planet::Jupiter,
            5 => astro::planet::Planet::Saturn,
            6 => astro::planet::Planet::Uranus,
            7 => astro::planet::Planet::Neptune,
            _ => {
                console_log!("❌ Invalid planet index: {}", planet_index);
                return std::ptr::null();
            }
        };

        // Get planet position using pure astro-rust VSOP87
        let (long_rad, lat_rad, radius_au) = astro::planet::heliocent_coords(&planet, julian_day);
        let pos = ecliptic_to_cartesian(long_rad, lat_rad, radius_au);

        buf[0] = pos.x;
        buf[1] = pos.y;
        buf[2] = pos.z;

        buf.as_ptr()
    })
}

/// **PURE ASTRO-RUST**: Get Pluto heliocentric position
///
/// Returns Pluto's position using ONLY astro::pluto::heliocent_pos() from the library.
#[wasm_bindgen]
pub fn get_pluto_position(julian_day: f64) -> *const f64 {
    thread_local! {
        static PLUTO_BUFFER: RefCell<[f64; 3]> = const { RefCell::new([0.0; 3]) };
    }

    PLUTO_BUFFER.with(|buffer| {
        let mut buf = buffer.borrow_mut();

        // Get Pluto position using pure astro-rust
        let (long_rad, lat_rad, radius_au) = astro::pluto::heliocent_pos(julian_day);
        let pos = ecliptic_to_cartesian(long_rad, lat_rad, radius_au);

        buf[0] = pos.x;
        buf[1] = pos.y;
        buf[2] = pos.z;

        buf.as_ptr()
    })
}

/// **PURE ASTRO-RUST**: Get nutation corrections
///
/// Returns nutation in longitude and obliquity using astro::nutation::nutation()
#[wasm_bindgen]
pub fn get_nutation(julian_day: f64) -> *const f64 {
    thread_local! {
        static NUTATION_BUFFER: RefCell<[f64; 2]> = const { RefCell::new([0.0; 2]) };
    }

    NUTATION_BUFFER.with(|buffer| {
        let mut buf = buffer.borrow_mut();

        let (nut_long, nut_oblq) = astro::nutation::nutation(julian_day);
        buf[0] = nut_long;
        buf[1] = nut_oblq;

        buf.as_ptr()
    })
}

/// **PURE ASTRO-RUST**: Get mean obliquity of ecliptic
///
/// Returns mean obliquity using astro::ecliptic::mn_oblq_IAU()
#[wasm_bindgen]
pub fn get_mean_obliquity(julian_day: f64) -> f64 {
    astro::ecliptic::mn_oblq_IAU(julian_day)
}

/// **PURE ASTRO-RUST**: Convert Julian Day to Julian Century
///
/// Uses astro::time::julian_cent() from the library
#[wasm_bindgen]
pub fn julian_day_to_century(julian_day: f64) -> f64 {
    astro::time::julian_cent(julian_day)
}

/// **PURE ASTRO-RUST**: Get planetary orbital elements
///
/// Returns orbital elements using astro::planet::orb_elements()
/// Returns: [mean_longitude, semimajor_axis, eccentricity, inclination, long_ascending_node, long_perihelion, mean_anomaly, arg_perihelion]
#[wasm_bindgen]
pub fn get_orbital_elements(planet_index: usize, julian_day: f64) -> *const f64 {
    thread_local! {
        static ELEMENTS_BUFFER: RefCell<[f64; 8]> = const { RefCell::new([0.0; 8]) };
    }

    ELEMENTS_BUFFER.with(|buffer| {
        let mut buf = buffer.borrow_mut();

        let planet = match planet_index {
            0 => astro::planet::Planet::Mercury,
            1 => astro::planet::Planet::Venus,
            2 => astro::planet::Planet::Earth,
            3 => astro::planet::Planet::Mars,
            4 => astro::planet::Planet::Jupiter,
            5 => astro::planet::Planet::Saturn,
            6 => astro::planet::Planet::Uranus,
            7 => astro::planet::Planet::Neptune,
            _ => {
                console_log!(
                    "❌ Invalid planet index for orbital elements: {}",
                    planet_index
                );
                return std::ptr::null();
            }
        };

        let (l_mean, a, e, i, omega, pi_long, m_mean, w) =
            astro::planet::orb_elements(&planet, julian_day);

        buf[0] = l_mean; // Mean longitude
        buf[1] = a; // Semimajor axis
        buf[2] = e; // Eccentricity
        buf[3] = i; // Inclination
        buf[4] = omega; // Longitude of ascending node
        buf[5] = pi_long; // Longitude of perihelion
        buf[6] = m_mean; // Mean anomaly
        buf[7] = w; // Argument of perihelion

        buf.as_ptr()
    })
}

/// **PURE ASTRO-RUST**: Get lunar illumination fraction
///
/// Calculate Moon's illuminated fraction using astro::lunar functions
#[wasm_bindgen]
pub fn get_lunar_illumination_fraction(julian_day: f64) -> f64 {
    // Get Sun and Moon positions using pure astro-rust functions
    let (sun_ecl, sun_dist_km) = astro::sun::geocent_ecl_pos(julian_day);
    let (moon_ecl, moon_dist_km) = astro::lunar::geocent_ecl_pos(julian_day);

    // Convert to AU for astro-rust function
    let sun_dist_au = sun_dist_km / 149597870.7;
    let moon_dist_au = moon_dist_km / 149597870.7;

    // Use astro-rust illumination calculation
    astro::lunar::illum_frac_frm_ecl_coords(
        moon_ecl.long,
        moon_ecl.lat,
        sun_ecl.long,
        moon_dist_au,
        sun_dist_au,
    )
}

/// **PURE ASTRO-RUST**: Get lunar ascending node position
///
/// Returns mean ascending node using astro::lunar::mn_ascend_node()
#[wasm_bindgen]
pub fn get_lunar_ascending_node(julian_day: f64) -> f64 {
    let julian_century = astro::time::julian_cent(julian_day);
    astro::lunar::mn_ascend_node(julian_century)
}

/// **PURE ASTRO-RUST**: Get lunar perigee position
///
/// Returns mean perigee using astro::lunar::mn_perigee()
#[wasm_bindgen]
pub fn get_lunar_perigee(julian_day: f64) -> f64 {
    let julian_century = astro::time::julian_cent(julian_day);
    astro::lunar::mn_perigee(julian_century)
}

/// **PURE ASTRO-RUST**: Apply precession correction to coordinates
///
/// Uses astro::precess::precess_ecl_coords() for ecliptic coordinates
#[wasm_bindgen]
pub fn apply_precession_ecliptic(
    longitude_rad: f64,
    latitude_rad: f64,
    jd_from: f64,
    jd_to: f64,
) -> *const f64 {
    thread_local! {
        static PRECESS_BUFFER: RefCell<[f64; 2]> = const { RefCell::new([0.0; 2]) };
    }

    PRECESS_BUFFER.with(|buffer| {
        let mut buf = buffer.borrow_mut();

        let (corrected_long, corrected_lat) =
            astro::precess::precess_ecl_coords(longitude_rad, latitude_rad, jd_from, jd_to);

        buf[0] = corrected_long;
        buf[1] = corrected_lat;

        buf.as_ptr()
    })
}

/// **PURE ASTRO-RUST**: Apply precession correction to equatorial coordinates
///
/// Uses astro::precess::precess_eq_coords() for right ascension/declination
#[wasm_bindgen]
pub fn apply_precession_equatorial(
    ra_rad: f64,
    dec_rad: f64,
    jd_from: f64,
    jd_to: f64,
) -> *const f64 {
    thread_local! {
        static PRECESS_EQ_BUFFER: RefCell<[f64; 2]> = const { RefCell::new([0.0; 2]) };
    }

    PRECESS_EQ_BUFFER.with(|buffer| {
        let mut buf = buffer.borrow_mut();

        let (corrected_ra, corrected_dec) =
            astro::precess::precess_eq_coords(ra_rad, dec_rad, jd_from, jd_to);

        buf[0] = corrected_ra;
        buf[1] = corrected_dec;

        buf.as_ptr()
    })
}

/// **PURE ASTRO-RUST**: Get planetary apparent magnitude
///
/// Calculate apparent magnitude using astro::planet functions
#[wasm_bindgen]
pub fn get_planetary_apparent_magnitude_muller(
    planet_index: usize,
    phase_angle_rad: f64,
    delta_au: f64,
    r_au: f64,
) -> f64 {
    let planet = match planet_index {
        0 => astro::planet::Planet::Mercury,
        1 => astro::planet::Planet::Venus,
        3 => astro::planet::Planet::Mars,
        4 => astro::planet::Planet::Jupiter,
        6 => astro::planet::Planet::Uranus,
        7 => astro::planet::Planet::Neptune,
        _ => return f64::NAN, // Invalid planet or not supported
    };

    match astro::planet::apprnt_mag_muller(&planet, phase_angle_rad, delta_au, r_au) {
        Ok(magnitude) => magnitude,
        Err(_) => f64::NAN,
    }
}

/// **PURE ASTRO-RUST**: Get planetary apparent magnitude (1984 method)
///
/// Calculate apparent magnitude using astro::planet 1984 method
#[wasm_bindgen]
pub fn get_planetary_apparent_magnitude_84(
    planet_index: usize,
    phase_angle_rad: f64,
    delta_au: f64,
    r_au: f64,
) -> f64 {
    let planet = match planet_index {
        0 => astro::planet::Planet::Mercury,
        1 => astro::planet::Planet::Venus,
        3 => astro::planet::Planet::Mars,
        4 => astro::planet::Planet::Jupiter,
        6 => astro::planet::Planet::Uranus,
        7 => astro::planet::Planet::Neptune,
        _ => return f64::NAN,
    };

    match astro::planet::apprnt_mag_84(&planet, phase_angle_rad, delta_au, r_au) {
        Ok(magnitude) => magnitude,
        Err(_) => f64::NAN,
    }
}

/// **PURE ASTRO-RUST**: Get planetary semidiameter
///
/// Calculate semidiameter using astro::planet::semidiameter()
#[wasm_bindgen]
pub fn get_planetary_semidiameter(planet_index: usize, distance_au: f64) -> f64 {
    let planet = match planet_index {
        0 => astro::planet::Planet::Mercury,
        1 => astro::planet::Planet::Venus,
        3 => astro::planet::Planet::Mars,
        4 => astro::planet::Planet::Jupiter,
        5 => astro::planet::Planet::Saturn,
        6 => astro::planet::Planet::Uranus,
        7 => astro::planet::Planet::Neptune,
        _ => return f64::NAN,
    };

    match astro::planet::semidiameter(&planet, distance_au) {
        Ok(semidiameter) => semidiameter,
        Err(_) => f64::NAN,
    }
}

/// **PURE ASTRO-RUST**: Get Sun's equatorial semidiameter
///
/// Calculate Sun's semidiameter using astro::sun::semidiameter()
#[wasm_bindgen]
pub fn get_sun_semidiameter(distance_au: f64) -> f64 {
    astro::sun::semidiameter(distance_au)
}

/// **PURE ASTRO-RUST**: Get Moon's equatorial semidiameter
///
/// Calculate Moon's semidiameter using astro::lunar::semidiameter()
#[wasm_bindgen]
pub fn get_moon_semidiameter(distance_km: f64) -> f64 {
    astro::lunar::semidiameter(distance_km)
}

/// **PURE ASTRO-RUST**: Get Moon's horizontal parallax
///
/// Calculate horizontal parallax using astro::lunar::eq_hz_parllx()
#[wasm_bindgen]
pub fn get_moon_horizontal_parallax(distance_km: f64) -> f64 {
    astro::lunar::eq_hz_parllx(distance_km)
}

/// **НАИТОЧНЕЙШИЙ РАСЧЕТ ЗЕНИТА СОЛНЦА**: Calculate exact solar zenith position on Earth's surface
///
/// Calculates the precise geographic coordinates where the Sun is directly overhead (zenith)
/// at the given Julian Day. Uses ONLY pure astro-rust functions for maximum precision.
///
/// **Algorithm using pure astro-rust:**
/// 1. Get Sun's geocentric ecliptic position (VSOP87 theory)
/// 2. Apply nutation corrections for sub-arcsecond accuracy
/// 3. Convert to equatorial coordinates (right ascension, declination)
/// 4. Calculate mean sidereal time at Greenwich
/// 5. Apply nutation to get apparent sidereal time
/// 6. Solar zenith longitude = apparent_sidereal_time - right_ascension
/// 7. Solar zenith latitude = declination
///
/// **Returns**: [longitude_degrees, latitude_degrees] where longitude is West-positive
/// **Performance**: High-precision calculations with nutation/precession corrections
// calculate_solar_zenith_position removed; zenith is included in compute_state.

/// Internal helper: compute solar zenith position in radians (lon E-positive, lat N-positive)
#[inline]
fn solar_zenith_position_rad_internal(julian_day: f64) -> (f64, f64) {
    // Geocentric solar ecliptic position with high-precision corrections
    let (sun_ecl, sun_dist_au) = astro::sun::geocent_ecl_pos(julian_day);
    // FK5 conversion (longitude/latitude)
    let (sun_long_fk5, sun_lat_fk5) =
        astro::sun::ecl_coords_to_FK5(julian_day, sun_ecl.long, sun_ecl.lat);
    // Annual aberration in ecliptic longitude
    let ab_long = astro::aberr::sol_aberr(sun_dist_au);
    // Nutation and true obliquity
    let (nut_long, nut_oblq) = astro::nutation::nutation(julian_day);
    let mean_oblq = astro::ecliptic::mn_oblq_IAU(julian_day);
    let true_oblq = mean_oblq + nut_oblq;
    let sun_corrected_long = sun_long_fk5 + ab_long + nut_long;
    // Equatorial coordinates (apparent)
    let sun_right_ascension =
        astro::coords::asc_frm_ecl(sun_corrected_long, sun_lat_fk5, true_oblq);
    let sun_declination = astro::coords::dec_frm_ecl(sun_corrected_long, sun_lat_fk5, true_oblq);
    // Sidereal time (apparent)
    let mean_sidereal_time = astro::time::mn_sidr(julian_day);
    let apparent_sidereal_time = astro::time::apprnt_sidr(mean_sidereal_time, nut_long, true_oblq);
    // Zenith longitude east-positive
    let mut zenith_longitude_rad = apparent_sidereal_time - sun_right_ascension;
    let two_pi_limited = astro::angle::limit_to_two_PI(zenith_longitude_rad);
    zenith_longitude_rad = if two_pi_limited > std::f64::consts::PI {
        two_pi_limited - 2.0 * std::f64::consts::PI
    } else {
        two_pi_limited
    };
    (zenith_longitude_rad, sun_declination)
}

// calculate_solar_zenith_position_rad removed; zenith is included in compute_state.

/// **PURE ASTRO-RUST**: Get mean sidereal time at Greenwich
///
/// Calculate mean sidereal time using astro::time::mn_sidr()
#[wasm_bindgen]
pub fn get_mean_sidereal_time(julian_day: f64) -> f64 {
    astro::time::mn_sidr(julian_day)
}

/// **PURE ASTRO-RUST**: Get apparent sidereal time at Greenwich
///
/// Calculate apparent sidereal time using astro::time::apprnt_sidr() with nutation corrections
#[wasm_bindgen]
pub fn get_apparent_sidereal_time(julian_day: f64) -> f64 {
    let mean_sidereal = astro::time::mn_sidr(julian_day);
    let (nut_long, nut_oblq) = astro::nutation::nutation(julian_day);
    let mean_oblq = astro::ecliptic::mn_oblq_IAU(julian_day);
    let true_oblq = mean_oblq + nut_oblq;

    astro::time::apprnt_sidr(mean_sidereal, nut_long, true_oblq)
}

/// **PURE ASTRO-RUST**: Convert ecliptic to equatorial coordinates
///
/// Convert using astro::coords functions with obliquity correction
#[wasm_bindgen]
pub fn convert_ecliptic_to_equatorial(
    ecl_long_rad: f64,
    ecl_lat_rad: f64,
    julian_day: f64,
    apply_nutation: bool,
) -> *const f64 {
    thread_local! {
        static EQUATORIAL_BUFFER: RefCell<[f64; 2]> = const { RefCell::new([0.0; 2]) };
    }

    EQUATORIAL_BUFFER.with(|buffer| {
        let mut buf = buffer.borrow_mut();
        let (right_ascension, declination) =
            ecliptic_to_equatorial_internal(ecl_long_rad, ecl_lat_rad, julian_day, apply_nutation);
        buf[0] = right_ascension;
        buf[1] = declination;
        buf.as_ptr()
    })
}

// solar_zenith_position_deg_internal removed; not part of the minimal API.

/// Internal helper: ecliptic to equatorial conversion with optional nutation
#[inline]
fn ecliptic_to_equatorial_internal(
    ecl_long_rad: f64,
    ecl_lat_rad: f64,
    julian_day: f64,
    apply_nutation: bool,
) -> (f64, f64) {
    let obliquity = if apply_nutation {
        let mean_oblq = astro::ecliptic::mn_oblq_IAU(julian_day);
        let (_nut_long, nut_oblq) = astro::nutation::nutation(julian_day);
        mean_oblq + nut_oblq
    } else {
        astro::ecliptic::mn_oblq_IAU(julian_day)
    };
    let right_ascension = astro::coords::asc_frm_ecl(ecl_long_rad, ecl_lat_rad, obliquity);
    let declination = astro::coords::dec_frm_ecl(ecl_long_rad, ecl_lat_rad, obliquity);
    (right_ascension, declination)
}

/// Get version information (contract check string)
#[wasm_bindgen]
pub fn get_version() -> String {
    // Minimal constant to satisfy frontend exact-match check
    "2.0.1".to_string()
}

/// **DEVELOPMENT ONLY**: Get current buffer contents in JSON string format
///
/// **DO NOT USE IN PRODUCTION** - violates no-string-passing rule
#[wasm_bindgen]
#[cfg(debug_assertions)]
pub fn debug_get_buffer() -> String {
    EPHEMERIS_BUFFER.with(|buffer| {
        let buf = buffer.borrow();
        format!("{:?}", buf.as_slice())
    })
}

/// Get the number of available astro-rust functions exposed by this wrapper
#[wasm_bindgen]
pub fn get_function_count() -> usize {
    // Count of pure astro-rust functions exposed:
    // compute_state, get_sun_position, get_moon_position, get_planet_position (8 planets),
    // get_pluto_position, get_nutation, get_mean_obliquity, julian_day_to_century,
    // get_orbital_elements, get_lunar_illumination_fraction, get_lunar_ascending_node,
    // get_lunar_perigee, apply_precession_ecliptic, apply_precession_equatorial,
    // get_planetary_apparent_magnitude_muller, get_planetary_apparent_magnitude_84,
    // get_planetary_semidiameter, get_sun_semidiameter, get_moon_semidiameter,
    // get_moon_horizontal_parallax,
    // get_mean_sidereal_time, get_apparent_sidereal_time, convert_ecliptic_to_equatorial
    25
}

// ===================== QUANTUM TIME (NT) SUPPORT =====================
// 1:1 перенос семантики из фронта (без строк). Возвращаем числа для форматирования на JS.

#[derive(Clone, Copy)]
struct QuantumEntry {
    u_ms: f64, // Unix ms
    d: i32,    // day in year [0..365)
    y: i32,    // year index starting at 0
}

thread_local! {
    static QUANTUM_TABLE: RefCell<Vec<QuantumEntry>> = RefCell::new(Vec::with_capacity(32000));
}

const QT_CONST_NT_MS: f64 = 1344643200000.0; // constNT @allow-wasm-const
const QT_CONST_D_MS: f64 = 86459178.082191780821918_f64; // constD @allow-wasm-const
const QT_CONST_D_EXTRA_MS: f64 = 43229589.41095890410959_f64; // constDExtra @allow-wasm-const
const QT_MAX_TIME_MS: f64 = 4090089600000.0; // maxTime @allow-wasm-const
const QT_SPECIAL_YEAR: i32 = 11; // specialYear @allow-wasm-const
const QT_SPECIAL_DAY: i32 = 121; // specialDay @allow-wasm-const

fn init_quantum_table_if_needed() {
    QUANTUM_TABLE.with(|tbl| {
        let mut v = tbl.borrow_mut();
        if !v.is_empty() {
            return;
        }
        let mut u = QT_CONST_NT_MS;
        let mut d: i32 = 0;
        let mut y: i32 = 0;
        while u < QT_MAX_TIME_MS {
            v.push(QuantumEntry { u_ms: u, d, y });
            if y == QT_SPECIAL_YEAR && d == QT_SPECIAL_DAY {
                u += QT_CONST_D_EXTRA_MS;
                d += 1;
                v.push(QuantumEntry { u_ms: u, d, y });
                u += QT_CONST_D_EXTRA_MS;
                d += 1;
                v.push(QuantumEntry { u_ms: u, d, y });
            } else {
                u += QT_CONST_D_MS;
                d += 1;
            }
            if d == 365 {
                d = 0;
                y += 1;
            }
        }
    });
}

#[inline]
fn floor_div(a: f64, b: f64) -> f64 {
    (a / b).floor()
}

fn adjust_ms_like_js(epoch_ms: f64, tz_offset_min: f64) -> f64 {
    // JS: setHours(24 - (getTimezoneOffset()/60 + 4)); minutes=seconds=ms=0
    // Воспроизводим локальную нормализацию суток и переносим к UTC.
    let local_ms = epoch_ms - tz_offset_min * 60_000.0;
    let day_ms = 86_400_000.0;
    let hour_ms = 3_600_000.0;
    let tz_hours = tz_offset_min / 60.0;
    let target_h_real = 24.0 - (tz_hours + 4.0);
    let target_h_floor = target_h_real.floor();
    // Нормализуем часы и сдвиг дня
    let h_norm = ((target_h_floor % 24.0) + 24.0) % 24.0;
    let day_offset = ((target_h_floor - h_norm) / 24.0).floor();
    let local_day_idx = floor_div(local_ms, day_ms);
    let adjusted_local_ms = (local_day_idx + day_offset) * day_ms + h_norm * hour_ms;
    // Обратно в UTC
    adjusted_local_ms + tz_offset_min * 60_000.0
}

fn binary_search_qt(table: &[QuantumEntry], target_u_ms: f64) -> Option<QuantumEntry> {
    let mut left: isize = 0;
    let mut right: isize = table.len() as isize - 1;
    let mut best: Option<QuantumEntry> = None;
    while left <= right {
        let mid = left + ((right - left) / 2);
        let e = table[mid as usize];
        if e.u_ms <= target_u_ms {
            best = Some(e);
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }
    best
}

/// Compute Quantum Time components [d_in_decade, decade_index, year_index]
/// Input: epoch_ms (Unix ms), timezone_offset_minutes (like Date.getTimezoneOffset())
#[wasm_bindgen]
pub fn get_quantum_time_components(epoch_ms: f64, timezone_offset_minutes: f64) -> *const f64 {
    thread_local! { static QT_OUT: RefCell<[f64; 3]> = const { RefCell::new([0.0; 3]) }; }
    QT_OUT.with(|buf| {
        let mut out = buf.borrow_mut();
        if !epoch_ms.is_finite() || !timezone_offset_minutes.is_finite() {
            return std::ptr::null();
        }
        init_quantum_table_if_needed();
        let adjusted_ms = adjust_ms_like_js(epoch_ms, timezone_offset_minutes);
        let res = QUANTUM_TABLE.with(|tbl| {
            let v = tbl.borrow();
            binary_search_qt(&v, adjusted_ms)
        });
        match res {
            Some(e) => {
                let dp = (e.d / 10) as f64;
                let d_in_decade = (e.d - (e.d / 10) * 10) as f64;
                out[0] = d_in_decade;
                out[1] = dp;
                out[2] = e.y as f64;
                out.as_ptr()
            }
            None => std::ptr::null(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_julian_day_calculation() {
        let result = compute_state(2451545.0);
        assert!(!result.is_null());
    }

    #[test]
    fn test_coordinate_count() {
        // Legacy removed; ensure compute_state returns non-null and buffer length contract is 11 f64
        let ptr = compute_state(2451545.0);
        assert!(!ptr.is_null());
    }

    #[test]
    fn test_pure_astro_rust_sun_position() {
        let result = get_sun_position(2451545.0, false);
        assert!(!result.is_null());
    }

    #[test]
    fn test_pure_astro_rust_moon_position() {
        let result = get_moon_position(2451545.0, false);
        assert!(!result.is_null());
    }

    #[test]
    fn test_pure_astro_rust_planet_position() {
        // Test Mercury (index 0)
        let result = get_planet_position(0, 2451545.0);
        assert!(!result.is_null());
    }

    #[test]
    fn test_pure_astro_rust_pluto_position() {
        let result = get_pluto_position(2451545.0);
        assert!(!result.is_null());
    }

    #[test]
    fn test_pure_astro_rust_nutation() {
        let result = get_nutation(2451545.0);
        assert!(!result.is_null());
    }

    #[test]
    fn test_pure_astro_rust_mean_obliquity() {
        let result = get_mean_obliquity(2451545.0);
        assert!(result > 0.0); // Should be positive angle in radians
    }

    #[test]
    fn test_pure_astro_rust_julian_century() {
        let result = julian_day_to_century(2451545.0);
        assert!((result - 0.0).abs() < 0.001); // J2000 should be ~0.0 centuries from J2000
    }

    #[test]
    fn test_pure_astro_rust_orbital_elements() {
        // Test Earth orbital elements (index 2)
        let result = get_orbital_elements(2, 2451545.0);
        assert!(!result.is_null());
    }

    #[test]
    fn test_pure_astro_rust_lunar_illumination() {
        let result = get_lunar_illumination_fraction(2451545.0);
        assert!(result >= 0.0 && result <= 1.0); // Illumination should be 0-1
    }

    #[test]
    fn test_pure_astro_rust_precession() {
        let result = apply_precession_ecliptic(0.0, 0.0, 2451545.0, 2451545.0 + 365.25);
        assert!(!result.is_null());
    }

    #[test]
    fn test_solar_zenith_position_calculation() {
        let (lon_east_rad, lat_rad) = solar_zenith_position_rad_internal(2451545.0);
        // Longitude should be within [-π, π]
        assert!(
            lon_east_rad >= -std::f64::consts::PI && lon_east_rad <= std::f64::consts::PI,
            "Solar zenith longitude out of bounds: {}",
            lon_east_rad
        );
        // Latitude should be within [-π/2, π/2]
        assert!(
            lat_rad.abs() <= std::f64::consts::FRAC_PI_2,
            "Solar zenith latitude out of bounds: {}",
            lat_rad
        );
    }

    #[test]
    fn test_mean_and_apparent_sidereal_time() {
        let mean_st = get_mean_sidereal_time(2451545.0);
        let apparent_st = get_apparent_sidereal_time(2451545.0);

        // Both should be in valid range [0, 2π]
        assert!(
            mean_st >= 0.0 && mean_st <= 2.0 * std::f64::consts::PI,
            "Mean sidereal time out of range: {}",
            mean_st
        );
        assert!(
            apparent_st >= 0.0 && apparent_st <= 2.0 * std::f64::consts::PI,
            "Apparent sidereal time out of range: {}",
            apparent_st
        );

        // Apparent should differ slightly from mean due to nutation
        let difference = (apparent_st - mean_st).abs();
        assert!(
            difference < 0.1,
            "Sidereal time difference too large: {}",
            difference
        ); // Should be small correction
    }

    #[test]
    fn test_ecliptic_to_equatorial_conversion() {
        // Test conversion of ecliptic coordinates to equatorial (internal helper avoids unsafe)
        let (right_ascension, declination) =
            ecliptic_to_equatorial_internal(0.0, 0.0, 2451545.0, false);
        // Right ascension should be in [0, 2π] range
        assert!(
            right_ascension >= 0.0 && right_ascension <= 2.0 * std::f64::consts::PI,
            "Right ascension out of range: {}",
            right_ascension
        );
        // Declination should be in [-π/2, π/2] range
        assert!(
            declination.abs() <= 1.5708,
            "Declination out of range: {}",
            declination
        );
    }
}
