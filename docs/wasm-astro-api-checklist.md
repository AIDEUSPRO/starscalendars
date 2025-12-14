## WASM Astronomical Core ↔ astro-rust API Checklist

**Цель**: зафиксировать строгую связь между экспортами `wasm-astro` и функциями `astro-rust`, чтобы:
- гарантировать отсутствие кастомных астрономических формул там, где есть готовые реализации в библиотеке;
- упростить ревью/аудит и дальнейшее расширение обёртки.

Файл относится к `wasm-astro/src/lib.rs` и `astro-rust/src/*`.

---

## 1. Горячий путь кадра

- **`compute_state(jd_utc_or_tt)`** → буфер `STATE[27]` (append-only)
  - **Слоты**:
    - `STATE[0..3)` — Sun: всегда `0,0,0` (сцена гелиоцентрическая, Солнце в центре; *нет* вызова astro здесь по дизайну горячего пути).
    - `STATE[3]` — Moon distance (геоцентрическое) в AU.
    - `STATE[4]` — Earth heliocentric Right Ascension (рад).
    - `STATE[5]` — Earth heliocentric Declination (рад).
    - `STATE[6]` — Earth–Sun расстояние (AU).
    - `STATE[7]` — Solar zenith longitude (рад, east+).
    - `STATE[8]` — Solar zenith latitude (рад, north+).
    - `STATE[9]` — Sublunar latitude (рад).
    - `STATE[10]` — Sublunar longitude (рад, east+).
    - `STATE[11..14)` — Earth-local unit vector toward Moon `[x,y,z]`.
    - `STATE[14]` — Apparent sidereal time (рад).
    - `STATE[15..27)` — zodiac/events append-only (см. `wasm-astro/src/lib.rs` doc-comment):
      - Sun/Moon ecliptic long/lat, illum fraction, elongation
      - zodiac indices (tropical + sidereal MVP=J2000)
      - node/perigee longitudes
      - phase8 id
  - **Используемые функции `astro-rust`**:
    - Нутация:
      - `astro::nutation::nutation(jd)` → `(nut_long, nut_oblq)`
    - Луна (геоцентрические эклиптические координаты, ELP-2000/82):
      - `astro::lunar::geocent_ecl_pos(jd)` → `(EclPoint { long, lat }, dist_km)`
    - Земля (гелиоцентрические эклиптические координаты, VSOP87):
      - `astro::planet::heliocent_coords(Planet::Earth, jd)` → `(long, lat, r_au)`
    - Солнечный зенит (внутренний helper, см. ниже; реализован как производная логика над astro-rust величинами):
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
    - Сублунарная точка (внутренний helper, производная логика над astro-rust величинами):
      - `compute_sublunar_position_internal(moon_long_with_nutation, moon_lat, true_oblq, apparent_sidereal)`:
        - `astro::coords::asc_frm_ecl` / `astro::coords::dec_frm_ecl`
    - Earth-local вектор к Луне (геометрия на сфере Земли; без эфемеридных формул):
      - `compute_earth_local_moon_direction(lat, lon_east)` → `[x,y,z]`
  - **Наша геометрия (допустимая)**:
    - Конверсия единиц km→AU (константа 149597870.7).
    - Нормализация долгот к диапазону \([-π, π]\) для lon_east (wrap-to-pi), когда нужно.

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

---

## 6. Zodiac + Lunar Events (active roadmap; research-first mapping)

> **Правило канона**: сначала ищем готовую реализацию в `astro-rust/src/*`. Если функции события нет (eclipses / void-of-course), допускается только **derived classifier/search** поверх величин, вычисленных через astro-rust.

### 6.1 Zodiac (Tropical)

- **Данные**: эклиптическая долгота \(\lambda\) в радианах.
- **Используемые функции `astro-rust`**:
  - Sun:
    - `astro::sun::geocent_ecl_pos(jd)` → `(EclPoint { long, lat }, dist_km)`
    - (для “аппаратной” долготы как в zenith pipeline) `astro::sun::ecl_coords_to_FK5`, `astro::aberr::sol_aberr`, `astro::nutation::nutation`
  - Moon:
    - `astro::lunar::geocent_ecl_pos(jd)` → `(EclPoint { long, lat }, dist_km)`
    - `astro::nutation::nutation(jd)` (для \(\lambda_\text{with nut}\))
- **Наша геометрия/классификация (допустимая)**:
  - `sign_index = floor(wrap_0_2pi(lambda) / (2π/12))` → `0..11`

### 6.2 Zodiac (Sidereal) — MVP definition (no external ayanamsa yet)

Пока **не вводим внешние ayanamsa таблицы/формулы**. Чтобы не нарушать правило “всё из astro-rust”, sidereal-версия в MVP определяется как:

- **Sidereal λ (MVP)**: \(\lambda_{J2000}\)
  - берем \((\lambda,\beta)\) на дату и **прецессируем эклиптические координаты назад на J2000.0**.
- **Используемые функции `astro-rust`**:
  - `astro::precess::precess_ecl_coords(lon, lat, jd_from, jd_to)`
    - `jd_to = 2451545.0` (J2000.0)
- **Примечание**: Lahiri/другие ayanamsa будут добавлены отдельным “special instructions” блоком, когда определим источник/модель и убедимся, что в `astro-rust` нет готового аналога.

### 6.3 Lunar phases (current + next events)

- **Текущая освещённость**:
  - `astro::lunar::illum_frac_frm_ecl_coords(moon_long, moon_lat, sun_long, earth_moon_dist, earth_sun_dist)` → fraction \([0,1]\)
- **Ближайшие четверти (off-frame)**:
  - `astro::lunar::Phase::{New,First,Full,Last}`
  - `astro::lunar::time_of_phase(date: &astro::time::Date, phase: &Phase) -> f64`
  - `astro::time::date_frm_julian_day(jd)` / `astro::time::julian_day(&date)` для связки JD↔Date
  - TT↔UTC: используем `wasm-astro::timescales` (TAI−UTC + 32.184s)
- **Возраст Луны (сутки, off-frame)**:
  - В `astro-rust` нет готовой “moon age days”, но есть точное время Новолуния (`time_of_phase(..., Phase::New)`), значит **возраст** вычисляем как:
    - `age_days = jd_tt_now - jd_tt_last_new`
  - В WASM экспорт: `moon_age_and_phase4(jd_utc) -> *const f64` возвращает `[age_days, phase4_id]` (phase4_id — последняя из 4 фаз, случившаяся до текущего времени).

### 6.4 Lunar nodes (ascending/descending)

- **Ближайшие проходы узлов (off-frame)**:
  - `astro::lunar::time_of_passage_through_nodes(date: &astro::time::Date) -> (jd_asc, jd_desc)`
- **Текущая долгота узла (для классификаторов событий)**:
  - `astro::lunar::true_ascend_node(JC)` (и/или `mn_ascend_node(JC)`)
  - где `JC = astro::time::julian_cent(jd)`

### 6.5 Apsides (perigee/apogee)

- В `astro-rust` есть:
  - `astro::lunar::mn_perigee(JC)` — **долгота** среднего перигея (не время события).
- **Время перигея/апогея**: в astro-rust готовой функции нет, значит реализуем как derived search:
  - базовая величина: расстояние Луны `moon_dist_km` из `astro::lunar::geocent_ecl_pos(jd)`
  - алгоритм: поиск локального минимума/максимума `dist(jd)` на окне вокруг старта (scan + refinement)

### 6.6 Eclipses (derived classifier/search)

В `astro-rust` нет `eclipse*` API, поэтому используем:
- кандидаты syzygy: `time_of_phase(... New/Full ...)`
- проверка близости к узлам/плоскости: значения из
  - `astro::lunar::geocent_ecl_pos(jd_event)` (moon lat)
  - `astro::lunar::true_ascend_node(JC)` (node long)
  - `astro::nutation::nutation(jd_event)` (если приводим долготы к одному “corrected” виду)
- классификация (порог) документируется в tz.md (как часть derived policy)

### 6.7 Void of course (derived classifier/search)

Событие астрологическое; в `astro-rust` готового API нет, поэтому делаем как derived classifier:
- Moon long/lat: `astro::lunar::geocent_ecl_pos(jd)`
- Геоцентрические **аппаратные** эклиптические долготы планет-таргетов:
  - `astro::planet::geocent_apprnt_ecl_coords(&Planet, jd)` → `(EclPoint { long, lat }, dist_au)`
  - при необходимости FK5: `astro::planet::ecl_coords_to_FK5(jd, long, lat)`
  - плюс `astro::nutation::nutation(jd)` если синхронизируем “corrected long”
- Derived logic:
  - найти время выхода Луны из текущего знака (root-find по `moon_long(t)` = boundary)
  - проверить, существует ли момент major aspect (0°, 60°, 90°, 120°, 180°) с любой планетой-таргетом до выхода из знака
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


