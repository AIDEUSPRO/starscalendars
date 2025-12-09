## WASM Astronomical Core ↔ astro-rust API Checklist

**Цель**: зафиксировать строгую связь между экспортами `wasm-astro` и функциями `astro-rust`, чтобы:
- гарантировать отсутствие кастомных астрономических формул там, где есть готовые реализации в библиотеке;
- упростить ревью/аудит и дальнейшее расширение обёртки.

Файл относится к `wasm-astro/src/lib.rs` и `astro-rust/src/*`.

---

## 1. Горячий путь кадра

- **`compute_state(jd_utc_or_tt)`** → буфер `STATE[14]`
  - **Слоты**:
    - `STATE[0..3)` — Sun: всегда `0,0,0` (сцена гелиоцентрическая, Солнце в центре; *нет* вызова astro здесь по дизайну горячего пути).
    - `STATE[3..6)` — Moon геоцентрические картезианские координаты (AU).
    - `STATE[6..9)` — Earth гелиоцентрические картезианские координаты (AU).
      - Конверсия RA/Dec использует `astro::coords::asc_frm_ecl`, `astro::coords::dec_frm_ecl`, `astro::ecliptic::mn_oblq_IAU`, `astro::nutation::nutation`.
    - `STATE[9]` — солнечный зенит, долгота (рад, восток положителен).
    - `STATE[10]` — солнечный зенит, широта (рад, север положителен).
    - `STATE[11]` — Earth heliocentric Right Ascension (рад).
    - `STATE[12]` — Earth heliocentric Declination (рад).
    - `STATE[13]` — Earth–Sun расстояние (AU).
  - **Используемые функции `astro-rust`**:
    - Нутация:
      - `astro::nutation::nutation(jd)` → `(nut_long, nut_oblq)`
    - Луна (геоцентрические эклиптические координаты, ELP-2000/82):
      - `astro::lunar::geocent_ecl_pos(jd)` → `(EclPoint { long, lat }, dist_km)`
    - Земля (гелиоцентрические эклиптические координаты, VSOP87):
      - `astro::planet::heliocent_coords(Planet::Earth, jd)` → `(long, lat, r_au)`
    - Солнечный зенит (внутренний helper, см. ниже):
      - `solar_zenith_position_rad_internal(jd)`:
        - `astro::sun::geocent_ecl_pos`
        - `astro::sun::ecl_coords_to_FK5`
        - `astro::aberr::sol_aberr`
        - `astro::nutation::nutation`
        - `astro::ecliptic::mn_oblq_IAU`
        - `astro::coords::asc_frm_ecl`
        - `astro::coords::dec_frm_ecl`
        - `astro::time::mn_sidr`
        - `astro::time::apprnt_sidr`
        - `astro::angle::limit_to_two_PI`
  - **Наша геометрия (допустимая)**:
    - `ecliptic_to_cartesian(long, lat, r)` — сферические → декартовы, в `astro-rust` такой функции нет.
    - Нормализация долгот к диапазону \([-π, π]\) после `limit_to_two_PI`.

---

## 2. Временные шкалы и события

### 2.1. Модуль `timescales` (UTC ↔ TT)

- **`timescales::utc_to_tt_jd(jd_utc)`**
  - `astro::time::date_frm_julian_day(jd_utc)` → `(year, month, day)`  
  - Далее используется локальная таблица leap seconds + формула:
    - `(TT−UTC) = (TAI−UTC) + 32.184 s`
    - Чисто доменный код, *не* дублирующий функциональность `astro-rust`.

- **`timescales::tt_to_utc_jd(jd_tt)`**
  - `astro::time::date_frm_julian_day(jd_tt)`
  - Аналогично: таблица leap seconds + `(TT−UTC)` как выше.

- **`set_tai_minus_utc_override(seconds)` / `clear_tai_minus_utc_override()`**
  - Без `astro` — чистый WASM-инфраструктурный уровень.

### 2.2. Солнцестояния

- **`next_winter_solstice_from(jd_utc_start)`**
  - **UTC → TT**:
    - `timescales::utc_to_tt_jd(jd_utc_start)`
  - **Аппаратная функция λ_app(TT)**:
    - `astro::sun::geocent_ecl_pos(jd_tt)` → `(sun_ecl, sun_dist_au)`
    - `astro::sun::ecl_coords_to_FK5(jd_tt, sun_ecl.long, sun_ecl.lat)`  
    - `astro::aberr::sol_aberr(sun_dist_au)`
    - `astro::nutation::nutation(jd_tt)`
    - `astro::angle::limit_to_two_PI`
  - **Численный решатель**:
    - Newton + численная производная по времени для решения λ_app(t) = 270°.
    - Это **наш** алгоритм верхнего уровня; в `astro-rust` нет готовых `next_*_solstice`.
  - **TT → UTC**:
    - `timescales::tt_to_utc_jd(jd_tt_event)`

### 2.3. Перигелий и афелий Земли

- **`earth_perihelion_aphelion_for_year_utc(year_utc)`**
  - **UTC дата → JD**:
    - `astro::time::Date { ... }`
    - `astro::time::julian_day(&date)` → `jd_start`
  - **Земная орбита**:
    - `astro::planet::heliocent_coords(Planet::Earth, jd)` → `(long, lat, r_au)`  
    - Обёртка-утилита `earth_r_lambda(jd)` возвращает `(r_au, long_mod_2pi)`.
  - **Поиск минимума/максимума r(jd)**:
    - Наш численный алгоритм: дневная выборка + параболическая аппроксимация вокруг лучшей точки (экстремум).
  - **TT → UTC**:
    - `timescales::tt_to_utc_jd(jd_tt_peri)`
    - `timescales::tt_to_utc_jd(jd_tt_aphel)`

---

## 3. “Чистые” обёртки вокруг astro-rust

Все функции ниже используют **только** указанные API `astro-rust` (плюс минимальную конверсию единиц/координат там, где в astro нет готового хелпера).

### 3.1. Позиции тел

- **`get_sun_position(jd, apply_nutation)`** → `*const [x,y,z] (AU)`
  - `astro::sun::geocent_ecl_pos(jd)` → `(EclPoint, dist_km)`
  - `astro::nutation::nutation(jd)` (если `apply_nutation == true`)
  - Конвертация km → AU локальной константой.
  - `ecliptic_to_cartesian(...)` — только геометрия.

- **`get_moon_position(jd, apply_nutation)`** → `*const [x,y,z] (AU)`
  - `astro::lunar::geocent_ecl_pos(jd)` → `(EclPoint, dist_km)`
  - `astro::nutation::nutation(jd)` (опционально)
  - km → AU + `ecliptic_to_cartesian`.

- **`get_planet_position(planet_index, jd)`** → `*const [x,y,z] (AU)`
  - Map `planet_index` → `astro::planet::Planet::{Mercury..Neptune}`.
  - `astro::planet::heliocent_coords(&planet, jd)` → `(long, lat, r_au)`
  - `ecliptic_to_cartesian`.

- **`get_pluto_position(jd)`** → `*const [x,y,z] (AU)`
  - `astro::pluto::heliocent_pos(jd)` → `(long, lat, r_au)`
  - `ecliptic_to_cartesian`.

### 3.2. Нутация, наклон, время

- **`get_nutation(jd)`** → `*const [nut_long, nut_oblq]`
  - `astro::nutation::nutation(jd)`

- **`get_mean_obliquity(jd)`** → `f64`
  - `astro::ecliptic::mn_oblq_IAU(jd)`

- **`julian_day_to_century(jd)`** → `f64`
  - `astro::time::julian_cent(jd)`

- **`get_mean_sidereal_time(jd)`** → `f64`
  - `astro::time::mn_sidr(jd)`

- **`get_apparent_sidereal_time(jd)`** → `f64`
  - `astro::time::mn_sidr(jd)` → `mean_sidereal`
  - `astro::nutation::nutation(jd)` → `(nut_long, nut_oblq)`
  - `astro::ecliptic::mn_oblq_IAU(jd)` → `mean_oblq`
  - `astro::time::apprnt_sidr(mean_sidereal, nut_long, true_oblq)`

### 3.3. Орбиты, прецессия

- **`get_orbital_elements(planet_index, jd)`** → `*const [8]`
  - Map индекса → `astro::planet::Planet`.
  - `astro::planet::orb_elements(&planet, jd)` → `(l_mean, a, e, i, omega, pi_long, m_mean, w)`.

- **`apply_precession_ecliptic(lon, lat, jd_from, jd_to)`** → `*const [lon',lat']`
  - `astro::precess::precess_ecl_coords(lon, lat, jd_from, jd_to)`

- **`apply_precession_equatorial(ra, dec, jd_from, jd_to)`** → `*const [ra',dec']`
  - `astro::precess::precess_eq_coords(ra, dec, jd_from, jd_to)`

### 3.4. Луна: освещённость, узлы, перигей

- **`get_lunar_illumination_fraction(jd)`** → `f64` в \([0,1]\)
  - `astro::sun::geocent_ecl_pos(jd)` → `sun_ecl, sun_dist_km`
  - `astro::lunar::geocent_ecl_pos(jd)` → `moon_ecl, moon_dist_km`
  - km → AU.
  - `astro::lunar::illum_frac_frm_ecl_coords(moon_ecl.long, moon_ecl.lat, sun_ecl.long, moon_dist_au, sun_dist_au)`

- **`get_lunar_ascending_node(jd)`** → `f64` (рад)
  - `astro::time::julian_cent(jd)` → `JC`
  - `astro::lunar::mn_ascend_node(JC)`

- **`get_lunar_perigee(jd)`** → `f64` (рад)
  - `astro::time::julian_cent(jd)` → `JC`
  - `astro::lunar::mn_perigee(JC)`

### 3.5. Преобразования координат

- **`convert_ecliptic_to_equatorial(long, lat, jd, apply_nutation)`** → `*const [ra,dec]`
  - Внутренний helper `ecliptic_to_equatorial_internal`:
    - `astro::ecliptic::mn_oblq_IAU(jd)` → `mean_oblq`
    - `astro::nutation::nutation(jd)` (если `apply_nutation`) → `nut_oblq`
    - `astro::coords::asc_frm_ecl`
    - `astro::coords::dec_frm_ecl`

---

## 4. Планетные величины (яркость, угловой размер)

- **`get_planetary_apparent_magnitude_muller(planet_index, phase_angle, delta_au, r_au)`** → `f64 | NaN`
  - Map индекса → `astro::planet::Planet` (поддержка только нужных тел).
  - `astro::planet::apprnt_mag_muller(&planet, phase_angle, delta_au, r_au)` → `Result<f64, _>`  
    Ошибки мапятся на `NaN`.

- **`get_planetary_apparent_magnitude_84(planet_index, phase_angle, delta_au, r_au)`** → `f64 | NaN`
  - `astro::planet::apprnt_mag_84(&planet, phase_angle, delta_au, r_au)` (аналогичная схема).

- **`get_planetary_semidiameter(planet_index, distance_au)`** → `f64 | NaN`
  - `astro::planet::semidiameter(&planet, distance_au)` → `Result<f64, _>`.

- **`get_sun_semidiameter(distance_au)`** → `f64`
  - `astro::sun::semidiameter(distance_au)`

- **`get_moon_semidiameter(distance_km)`** → `f64`
  - `astro::lunar::semidiameter(distance_km)`

- **`get_moon_horizontal_parallax(distance_km)`** → `f64`
  - `astro::lunar::eq_hz_parllx(distance_km)`

---

## 5. Quantum Time (NT) и служебные функции

Эти функции **не обязаны** использовать `astro-rust`; они реализуют доменный уровень и служебный контракт.

- **`get_quantum_time_components(epoch_ms, tz_offset_min)`** → `*const [d_in_decade, decade_index, year_index]`
  - Внутренние helpers:
    - `init_quantum_table_if_needed()` — прогон по временной оси [NT-эпоха..maxTime] с добавлением спец-дней.
    - `adjust_ms_like_js(...)` — воспроизводит JS-логику нормализации суток.
    - `binary_search_qt(...)` — поиск в таблице QuantumEntry.
  - **Не использует `astro-rust`**, специально.

- **`get_version()`**, **`debug_get_buffer()` (debug only)**, **`get_function_count()`**
  - Чисто служебные/контрактные, без астрономических расчётов.

---

## 6. Внутренние helper-ы (собственная математика, допустимая)

Эти функции **намеренно не используют astro-rust**, потому что:
- либо на этом уровне в `astro-rust` просто нет подходящей абстракции (только сферические координаты, нет декартовых);
- либо это чисто численные/вспомогательные вещи, не относящиеся к эфемеридам.

- `ecliptic_to_cartesian(long, lat, r)` — сферические ecliptic → декартовы координаты (AU).
- `solar_zenith_position_rad_internal(jd)`:
  - Вся физическая часть на `astro::sun/*`, `astro::aberr`, `astro::nutation`, `astro::ecliptic`, `astro::coords`, `astro::time`, `astro::angle`.
  - Своя часть — только нормализация долготы к \([-π, π]\).
- `ecliptic_to_equatorial_internal(..)`:
  - Физика (`mn_oblq_IAU`, `nutation`, `asc_frm_ecl`, `dec_frm_ecl`) — через `astro`.
  - Своя часть отсутствует, только связка вызовов.
- `timescales::*`, Quantum Time helpers, бинарный поиск, Newton-решатели, ограничители шага/диапазона — верхнеуровневая численная логика поверх `astro-rust`.

---

## 7. Инвариант “без самодельных формул” (проверочный чеклист)

Для ревью новых функций в `wasm-astro/src/lib.rs`:

1. **Есть ли эквивалентная функция в `astro-rust`?**
   - Если да — использовать **только** её, не переписывая формулу.
   - Если нет — допускается:
     - численный решатель (Newton/сканирование) поверх существующих координат/величин;
     - простая геометрия (сферические→декартовы, нормализация углов).
2. **Вся “астрономия” (позиции, нутация, прецессия, освещённость, узлы, перигей, яркость, размеры) должна исходить из `astro::*`.**
3. **Константы**:
   - Разрешены только:
     - единичные физические/геометрические (AU в км, миллисекунды в дне и т.п.);
     - доменные (NT-эпоха, спец-дни в Quantum Time);
   - Нельзя хардкодить мировые эфемеридные константы, если они уже есть в `astro-rust`.
4. **При добавлении новой обёртки**:
   - Явно задокументировать в этом файле:
     - список вызываемых `astro`-функций;
     - объём собственной математики (и почему она не дублирует astro-rust).


