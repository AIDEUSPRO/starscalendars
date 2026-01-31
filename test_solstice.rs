// Test winter solstice accuracy
// Compile: rustc test_solstice.rs -L target/debug/deps
// Run: ./test_solstice

extern crate astro;

fn main() {
    // Test for winter solstice 2024
    // According to astronomical sources, winter solstice 2024 is on
    // December 21, 2024 at 09:21 UTC
    
    // Convert December 1, 2024 00:00 UTC to Julian Day
    let dec_1_2024 = astro::time::julian_day(&astro::time::Date {
        year: 2024,
        month: 12,
        decimal_day: 1.0,
        cal_type: astro::time::CalType::Gregorian,
    });
    
    println!("Starting from December 1, 2024: JD = {:.6}", dec_1_2024);
    
    // Simulate the algorithm from wasm-astro/src/lib.rs
    let result = find_winter_solstice(dec_1_2024);
    
    println!("\nFound winter solstice at JD UTC: {:.6}", result);
    
    // Convert back to calendar date
    match astro::time::date_frm_julian_day(result) {
        Ok((year, month, day)) => {
            let hours = (day.fract() * 24.0) as i32;
            let minutes = ((day.fract() * 24.0 - hours as f64) * 60.0) as i32;
            let seconds = ((day.fract() * 24.0 * 60.0 - (hours * 60 + minutes) as f64) * 60.0) as i32;
            
            println!("Winter solstice 2024: {}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
                     year, month, day.floor() as i32, hours, minutes, seconds);
            
            // Expected: December 21, 2024 09:21 UTC
            println!("\nExpected: 2024-12-21 09:21:00 UTC");
            
            // Calculate accuracy
            let expected_jd = astro::time::julian_day(&astro::time::Date {
                year: 2024,
                month: 12,
                decimal_day: 21.0 + (9.0 + 21.0/60.0) / 24.0,
                cal_type: astro::time::CalType::Gregorian,
            });
            
            let diff_minutes = (result - expected_jd) * 24.0 * 60.0;
            println!("Difference from expected: {:.1} minutes", diff_minutes);
        }
        Err(e) => println!("Error converting JD to date: {:?}", e),
    }
}

fn find_winter_solstice(jd_utc_start: f64) -> f64 {
    // Helper: convert UTC JD to TT JD using ΔT(year, month)
    let to_tt = |jd: f64| -> f64 {
        let (year, month, _day) = match astro::time::date_frm_julian_day(jd) {
            Ok((y, m, d)) => (y as i32, m as u8, d),
            Err(_) => return f64::NAN,
        };
        let delta_t_sec = astro::time::delta_t(year, month);
        astro::time::julian_ephemeris_day(jd, delta_t_sec)
    };

    // Solar declination δ⊙ (apparent) at TT JD
    let solar_decl_tt = |jd_tt: f64| -> f64 {
        let (nut_long, nut_oblq) = astro::nutation::nutation(jd_tt);
        let mean_oblq = astro::ecliptic::mn_oblq_IAU(jd_tt);
        let true_oblq = mean_oblq + nut_oblq;
        let (sun_ecl, _sun_dist_km) = astro::sun::geocent_ecl_pos(jd_tt);
        let corrected_long = sun_ecl.long + nut_long;
        astro::coords::dec_frm_ecl(corrected_long, sun_ecl.lat, true_oblq)
    };

    // Start from TT corresponding to start UTC
    let jd_tt0 = to_tt(jd_utc_start);
    if !jd_tt0.is_finite() {
        return f64::NAN;
    }

    // Coarse scan forward up to ~200 days to find vicinity of minimum
    let mut best_jd = jd_tt0;
    let mut best_val = f64::INFINITY;
    let mut jd = jd_tt0;
    let end = jd_tt0 + 200.0;
    while jd <= end {
        let v = solar_decl_tt(jd);
        if v < best_val {
            best_val = v;
            best_jd = jd;
        }
        jd += 1.0;
    }

    println!("Coarse scan found minimum near JD TT: {:.6} with declination: {:.6} rad", best_jd, best_val);

    // Refine around best_jd with ternary search
    let mut a = best_jd - 5.0;
    let mut b = best_jd + 5.0;
    for iteration in 0..40 {
        let m1 = a + (b - a) / 3.0;
        let m2 = b - (b - a) / 3.0;
        let f1 = solar_decl_tt(m1);
        let f2 = solar_decl_tt(m2);
        if f1 < f2 {
            b = m2;
        } else {
            a = m1;
        }
        
        if iteration == 39 {
            println!("After 40 iterations of ternary search:");
            println!("  Interval: [{:.10}, {:.10}]", a, b);
            println!("  Width: {:.10} days = {:.2} seconds", b - a, (b - a) * 86400.0);
        }
    }
    let jd_tt_min = (a + b) / 2.0;
    
    let final_decl = solar_decl_tt(jd_tt_min);
    println!("Final minimum at JD TT: {:.10} with declination: {:.10} rad = {:.6}°", 
             jd_tt_min, final_decl, final_decl.to_degrees());

    // Convert TT -> UTC using ΔT at event date
    let (year_ev, month_ev, _day_ev) = match astro::time::date_frm_julian_day(jd_tt_min) {
        Ok((y, m, d)) => (y as i32, m as u8, d),
        Err(_) => return f64::NAN,
    };
    let delta_t_sec_ev = astro::time::delta_t(year_ev, month_ev);
    println!("ΔT for {}/{}: {:.2} seconds", year_ev, month_ev, delta_t_sec_ev);
    
    let jd_utc_ev = jd_tt_min - (delta_t_sec_ev / 86400.0);

    jd_utc_ev
}