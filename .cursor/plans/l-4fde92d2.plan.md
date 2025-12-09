<!-- 4fde92d2-cc16-4c2f-bf14-c5fa2bda6426 b84e82ad-6b35-444a-9236-d2290b38c488 -->
# План: финальная оптимизация STATE буфера (15 элементов)

### Цели

- Убрать все неиспользуемые Cartesian координаты (Earth и Moon) из WASM hot path.
- Оставить только необходимые величины: расстояния, RA/Dec, sublunar координаты, Earth-local вектор.
- Минимизировать размер STATE буфера до 15 элементов (было 14, но добавляем нужные данные без дубликатов).

### Финальный layout STATE (15 f64)

- Увеличить `STATE_BUFFER` с `[f64; 14]` до `[f64; 15]`.
- Layout:
- `0..2`  — Sun zeros (x,y,z = 0).
- `3`     — Moon distance геоцентрическое (AU).
- `4`     — Earth heliocentric RA (рад).
- `5`     — Earth heliocentric Dec (рад).
- `6`     — Earth–Sun расстояние (AU).
- `7`     — Solar zenith долгота (рад, восток +).
- `8`     — Solar zenith широта (рад, север +).
- `9`     — Moon sublunar широта (рад).
- `10`    — Moon sublunar долгота (рад, восток +).
- `11..13` — Earth-local unit vector toward Moon [x, y, z].
- `14`    — Apparent sidereal time (рад).

### Шаги по wasm-astro (`wasm-astro/src/lib.rs`)

1. Удалить Earth Cartesian из `compute_state`

- Убрать расчёт `earth_pos = ecliptic_to_cartesian(...)` и запись в `out[6..8]`.
- Сохранить расчёт `earth_long, earth_lat, earth_r` (нужны для RA/Dec и дистанции).
- Писать:
- `out[4]` = Earth RA (через `asc_frm_ecl` + `limit_to_two_PI`).
- `out[5]` = Earth Dec.
- `out[6]` = Earth–Sun расстояние `earth_r`.

2. Удалить Moon Cartesian, заменить на расстояние

- Убрать расчёт `moon_pos = ecliptic_to_cartesian(...)` и запись в `out[3..5]`.
- Сохранить расчёт `moon_ecl, moon_dist_km` из `astro::lunar::geocent_ecl_pos(jd)`.
- Писать:
- `out[3]` = `moon_dist_km / 149597870.7` (Moon distance в AU).

3. Единоразовый набор общих астрономических величин

- После валидации JD вычислить один раз:
- `nut_long`, `nut_oblq` = `astro::nutation::nutation(jd)`.
- `mean_oblq` = `astro::ecliptic::mn_oblq_IAU(jd)`.
- `true_oblq = mean_oblq + nut_oblq`.
- `mean_sidereal` = `astro::time::mn_sidr(jd)`.
- `apparent_sidereal` = `astro::time::apprnt_sidr(mean_sidereal, nut_long, true_oblq)`.
- Использовать эти значения для Земли, сублунарной точки и записи AST в `out[14]` без повторных вызовов.

4. Добавить вычисление sublunar и Earth-local вектора

- Helper `compute_sublunar_position_internal(moon_corr_long, moon_ecl.lat, true_oblq, apparent_sidereal) -> (lat_rad, lon_east_rad)`:
- RA/Dec Луны через `asc_frm_ecl`/`dec_frm_ecl` с `true_oblq`.
- `lon_east = wrap_to_pi(apparent_sidereal - moon_ra)` в диапазон `[-π, π]`.
- `lat = moon_dec`.
- Записать `lat`/`lon_east` в `out[9]`/`out[10]`.
- Helper `compute_earth_local_moon_direction(lat_rad, lon_east_rad) -> (x,y,z)`:
- `phi = π/2 - lat_rad`, `theta = -lon_east_rad + π`.
- `sin_phi = sin(phi)`, `x = sin_phi * cos(theta)`, `y = cos(phi)`, `z = sin_phi * sin(theta)`.
- Записать `(x,y,z)` в `out[11]..[13]`.
- Записать `apparent_sidereal` в `out[14]`.

5. Обновить doc-комментарий над `compute_state`

- Зафиксировать новый layout из 15 элементов и описание каждого слота.

### Шаги по фронтенду (`frontend/src/scene/BabylonScene.tsx`)

1. Обновить размер и layout STATE

- Ввести `const STATE_STRIDE = 15;` в начале файла.
- Заменить все `new Float64Array(..., ptr, 14)` на `STATE_STRIDE`.
- Обновить комментарий о layout буфера.

2. Убрать использование Earth Cartesian

- Удалить строку `const ex = buf[6]!, ey = buf[7]!, ez = buf[8]!;` (не используется).
- Обновить чтение Earth данных:
- `earthRaRad = buf[4]!;`
- `earthDecRad = buf[5]!;`
- `earthDistanceAu = buf[6]!;`

3. Перевести Луну на новые поля STATE

- В `updateCelestialPositionsRealtime` в блоке Луны:
- Заменить `const mxAU = buf[3]!; const myAU = buf[4]!; const mzAU = buf[5]!;` на:
- `const moonDistanceAu = buf[3]!;`
- `const rUnits = moonDistanceAu * MOON_UNITS_PER_AU;`
- Вместо `computeSublunarLatLonDeg` использовать:
- `const sublunarLatRad = buf[9]!;`
- `const sublunarLonRad = buf[10]!;`
- `const moonLocalX = buf[11]!; const moonLocalY = buf[12]!; const moonLocalZ = buf[13]!;`
- Позиция Луны:
- Transform Earth-local `(moonLocalX, moonLocalY, moonLocalZ)` через `earthPivot.rotationQuaternion` как сейчас.
- Зеленый сублунарный маркер:
- Использовать `sublunarLatRad`/`sublunarLonRad` в текущей формуле для `(xL, yL, zL)`.
- Полностью удалить функцию `computeSublunarLatLonDeg` и вызовы `wasm.get_mean_obliquity`/`wasm.get_apparent_sidereal_time`.

4. Проверить все места использования STATE

- Найти все места, где читается STATE буфер (орбита Земли, debug логи и т.п.).
- Обновить индексы согласно новому layout.

### Документация

1. Обновить `docs/wasm-astro-api-checklist.md`

- В секции "Горячий путь кадра" зафиксировать новый layout STATE[15].
- Указать, что Earth и Moon Cartesian больше не возвращаются.
- Описать используемые astro-функции для sublunar геометрии.

2. Обновить `tz.md` (если нужно)

- Уточнить, что STATE теперь содержит только необходимые величины без дубликатов.

### Проверки

1. WASM сборка и тесты

- Собрать `wasm-astro` и проверить, что всё компилируется.
- Запустить `pnpm run test:wasm` для проверки загрузки модуля.

2. Визуальная проверка сцены

- Проверить, что Земля позиционируется корректно через RA/Dec.
- Проверить, что Луна движется вокруг Земли корректно.
- Проверить, что зелёный сублунарный маркер совпадает с направлением на Луну.
- Проверить при перемотке времени, что нет рассинхрона.

### To-dos

- [ ] Убрать вычисление и запись Earth и Moon Cartesian координат из compute_state, заменить на расстояния и RA/Dec.
- [ ] Добавить вычисление sublunar lat/lon и Earth-local вектора Луны в compute_state без повторных astro-вызовов.
- [ ] Обновить frontend на новый layout STATE[15]: убрать чтение Cartesian, использовать новые индексы для Earth RA/Dec/dist и Moon distance/sublunar/vector.
- [ ] Удалить computeSublunarLatLonDeg и все вызовы get_mean_obliquity/get_apparent_sidereal_time из hot path.
- [ ] Обновить docs/wasm-astro-api-checklist.md и tz.md с новым компактным layout STATE[15].