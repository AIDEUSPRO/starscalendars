Ниже — детализированное ТЗ уровня профессионального продакшна для высоконагруженной, низколатентной системы реального времени. Основано на чистой архитектуре, современном Rust 1.89+ (минимальная версия для edition 2024), идиоматических практиках, без антипаттернов. Включает ключевые библиотеки на момент подготовки и примеры кода. VR пока не реализуем, но вся архитектура VR-ready.

## Цель
Создать кроссплатформенное веб-приложение астрономической визуализации с 3D‑сценой (Babylon.js 8) и покадровыми расчетами эфемерид в Rust/WASM. Бэкенд на Axum обеспечивает аутентификацию (JWT), интеграцию с Telegram (проверка подписки), хранение в PostgreSQL и двунаправленный обмен по WebSocket. Личный кабинет/админка/авторизация вынесены в отдельное Dioxus WASM-приложение. **Система поддерживает 10 языков с культурными адаптациями для глобального духовного сообщества.** Стек и код соответствуют принципам чистой архитектуры, SOLID, явному управлению зависимостями, тестируемости и высокой производительности.

## 🌟 Астрономическая система координат (КРИТИЧЕСКИ ВАЖНО)

**МОДЕЛЬ КООРДИНАТ:**
- **ГЕЛИОЦЕНТРИЧЕСКАЯ для планет**: Солнце в центре сцены (0,0,0), все планеты (включая Землю) используют VSOP87 гелиоцентрические координаты и вращаются вокруг Солнца
- **ГЕОЦЕНТРИЧЕСКАЯ для Луны**: Луна использует ELP-2000/82 геоцентрические координаты относительно позиции Земли

**🚨 КРИТИЧЕСКИ ВАЖНО - РЕАЛИЗАЦИЯ В WASM С ПОЛНЫМ ПОКРЫТИЕМ ASTRO-RUST:**
- **✅ СТАТУС**: обертка полностью покрывает astro-rust (ориентир ~24 функций); для сцены используется только единый `compute_state(jd)`
- **🌟 СОЛНЕЧНЫЙ ЗЕНИТ**: Возвращается в составе буфера `compute_state()` (lon_east_rad, lat_rad)
 - **🔒 КАНОН**: Маркер зенита на Земле вычисляется ТОЛЬКО из этих радиан без дополнительных поправок. Схема:
   - Сфера (локально к Земле): `phi=(π/2)-lat_rad`, `theta=(-lon_east_rad)+π`
   - Поворот пивота: `pivot.y = -((-lon_east_rad)+π)`, `pivot.z = lat_rad`, `pivot.x = lat_rad`
   - Меш Земли не вращается; Луна/орбита следуют пивоту
   - Единичный RH→LH Z‑flip применяется только к мировым позициям тел, не к маркеру
- **🔥 СТРОГИЙ ЗАПРЕТ**: Любые mock-данные, отсебятина или кастомные формулы АБСОЛЮТНО ЗАПРЕЩЕНЫ
- Солнце: статически в позиции (0,0,0) - центр системы
- Земля: позиция рассчитывается ТОЛЬКО через astro-rust VSOP87 (гелиоцентрическая орбита)
- Луна: позиция рассчитывается ТОЛЬКО через astro-rust ELP-2000/82 (геоцентрическая орбита) + смещение к позиции Земли
Примечание (оптимизация горячего пути): в `wasm-astro/src/lib.rs` слоты Солнца [0..2] в выходном STATE‑буфере заполняются нулями преднамеренно, поскольку сцена гелиоцентрическая и Солнце всегда закреплено в мировом (0,0,0). Это избавляет от ненужных вычислений солнечной позиции в кадре. Зенит при этом рассчитывается точно (нутация/обликвитет/сидерическое время) и включён в буфер. Для расчёта дат событий (например, солнцестояний) предусмотрена отдельная функция `next_winter_solstice_from(jd_utc_start)` — офф‑фрейм, реализована как решатель λ_app(t)=270° (FK5 + годичная аберрация + нутация) в шкале TT, затем конверсия в UTC через модуль timescales (TAI−UTC + 32.184s). Кешировать результат.
- Остальные планеты: ТОЛЬКО VSOP87 гелиоцентрические координаты из astro-rust
- **🛡️ ГАРАНТИЯ КАЧЕСТВА**: Все астрономические расчеты используют ИСКЛЮЧИТЕЛЬНО функции библиотеки astro-rust

**ВИЗУАЛИЗАЦИЯ В 3D СЦЕНЕ (Babylon.js 8):**
 - **Система координат**: используем левостороннюю систему Babylon (значение по умолчанию). Астрономические данные из WASM остаются в научной RH системе; единственный RH→LH Z‑flip выполняется в сцене при присвоении позиций. Не включаем `useRightHandedSystem`.
- **Кинематографическая сцена** с Солнцем как источником света в центре (0,0,0)
- **Земля** видимо вращается вокруг Солнца по эллиптической орбите (VSOP87 гелиоцентрические координаты)
- **Луна** видимо вращается вокруг Земли по своей орбите (ELP-2000/82 геоцентрические координаты + смещение к Земле)
  - На сцене позиция Луны и сублунарный маркер вычисляются из единого астрономического источника: RA/Dec Луны + видимое сидерическое время (AST). Это устраняет долговременный сдвиг долготы; широта/долгота совпадают с внешними источниками
- **Камера** (ArcRotateCamera) по умолчанию смотрит на Землю для лучшего обзора системы
- **Масштабирование**: 700 ед/1 AU (как в референсной сцене) для орбитальной сцены. Размеры тел (DIAMETER семантика для Babylon mesh): Earth=50, Moon=20, Sun=40. ENV_H=2 для оболочки облаков.

Высокоуровневая архитектура (чистая)
Слои и зависимости (внешние слои зависят от внутренних только через интерфейсы/абстракции):

Domain (ядро): чистые типы и бизнес‑правила (эфемеридная модель, валидаторы, политики авторизации).
UseCases/Application (сервисные интеракторы): оркестровка сценариев (логин, привязка Telegram, ревалидация статуса подписки, выдача JWT). Зависит от портов (traits) репозиториев/шлюзов.
Adapters/Interfaces: реализации портов (репозитории SQLX, Telegram Bot API, кэш), мапперы DTO, валидаторы ввода/вывода.
Delivery (входные/выходные интерфейсы): Axum HTTP/WS, Dioxus server functions, Vite/TS фронтенд, WASM интерфейс.
Cross-cutting: observability (tracing), конфигурация (env+typed), безопасность (JWT, CSP), миграции.
Монорепозиторий (pnpm + Cargo workspaces)
root/

frontend/ (TypeScript + Vite + Babylon.js)
wasm-astro/ (Rust → WASM: эфемеридное ядро - ПОЛНОЕ ПОКРЫТИЕ astro-rust с ЗАПРЕТОМ на mock-данные)
backend/ (Axum HTTP/WS, PostgreSQL, Telegram, JWT)
dioxus-app/ (Dioxus fullstack для auth/profile/admin)
libs/
domain/ (общие доменные типы и контракты)
app/ (use-cases, портовые интерфейсы)
infra/ (клиенты PostgreSQL/Telegram/Cache, реализация портов)
ops/ (миграции, Helm/compose, CI/CD) - МЫ НЕ ИСПОЛЬЗУЕМ ДОКЕР И РУКАМИ РАЗВОРАЧИВАЕМ НА СЕРВЕР Almalinux 9.4 уже скомпилированны фронт и только сарвер компилируем на своем сервере линукс для продакшна к которому копируем скомпилированный фронт!!!
## 🚨 КРИТИЧЕСКИЕ ПРАВИЛА РАЗРАБОТКИ WASM ОБЕРТКИ

### **СТРОГИЕ ОГРАНИЧЕНИЯ (НАРУШЕНИЕ = ПРОВАЛ ПРОЕКТА):**

**1. 🚫 АБСОЛЮТНО ЗАПРЕЩЕНО:**
- **Mock-данные любого вида** - даже временные или для тестов
- **Любая отсебятина** или кастомные астрономические расчеты
- **Hardcoded значения** планетарных позиций или констант
- **Математические формулы не из astro-rust библиотеки**
- **eval()** в любом контексте - критическая уязвимость безопасности
- **Изменение кода в папке `./astro-rust/`** - она read-only!
- **Частичное покрытие API** - должны быть ВСЕ функции библиотеки

**2. ✅ ОБЯЗАТЕЛЬНО ИСПОЛЬЗОВАТЬ:**
- **ТОЛЬКО функции из astro-rust** для всех астрономических расчетов
- **compute_state(jd) один раз на кадр** (буфер 11 f64: Sun xyz, Moon xyz, Earth xyz, Zenith [lon_east_rad, lat_rad])
- **Zero-copy data transfer** через Float64Array и thread-local буферы
- **Максимальная точность** с коррекциями нутации/прецессии/аберрации при необходимости

### Reusable time scales (UTC↔TT)
- В `wasm-astro` добавлен модуль `timescales`: UTC↔TT по формуле (TT−UTC)=(TAI−UTC)+32.184s
- Таблица leap seconds встроена (последний 2017‑01=37s); есть WASM‑сетторы для override без пересборки
- Все функции событий обязаны использовать этот модуль, не `ΔT` для UTC↔TT

### Quantum Time (NT)
- JS реализация NT перенесена в WASM: `get_quantum_time_components(epoch_ms, tz_offset_min)`
- Возвращает три числа для форматирования UI; обновление метки раз в минуту
- Семантика идентична прежней: специальные дни, фиксированные длительности
- **Production-ready паттерны** Rust 1.89+ с WASM-bindgen

**3. 🎯 ПРИМЕР ПРАВИЛЬНОЙ РЕАЛИЗАЦИИ НОВОЙ ФУНКЦИИ:**
```rust
// ✅ ПРАВИЛЬНО - только astro-rust функции
#[wasm_bindgen]
// Зенит включён в compute_state; отдельная функция не используется в сцене
    // Используем ТОЛЬКО astro::sun::geocent_ecl_pos()
    let (sun_ecl, _) = astro::sun::geocent_ecl_pos(julian_day);
    // Применяем ТОЛЬКО astro::nutation::nutation()
    let (nut_long, nut_oblq) = astro::nutation::nutation(julian_day);
    // И так далее - ТОЛЬКО библиотечные функции
}

// ❌ ЗАПРЕЩЕНО - любая отсебятина
#[wasm_bindgen] 
pub fn bad_solar_position(julian_day: f64) -> *const f64 {
    let fake_x = 1.0; // ❌ Mock данные!
    let custom_calc = julian_day * 0.123; // ❌ Кастомная формула!
}
```

**4. 📊 ТЕКУЩЕЕ СОСТОЯНИЕ WASM ОБЕРТКИ:**
- ✅ Полное покрытие astro-rust API (ориентир ~24 функций); горячий путь — `compute_state(jd)`
- ✅ **Солнечный зенит** реализован для точного поворота Земли
- ✅ **Нутация и прецессия** включены для максимальной точности
- ✅ **Нулевое копирование** данных через thread-local буферы
- ✅ **Производительность O(1)** для горячего пути рендеринга
- 🛡️ **Гарантия**: никаких mock-данных или отсебятины

### **🔒 ДЛЯ ВСЕХ РАЗРАБОТЧИКОВ - КРИТИЧЕСКИ ВАЖНО:**
- При добавлении новых функций в WASM: используй ТОЛЬКО astro-rust API
- Никогда не изменяй файлы в папке `./astro-rust/` - она неприкосновенна
- Все астрономические расчеты должны быть из библиотеки astro-rust
- Mock-данные запрещены даже в тестах - используй реальные расчеты
- При сомнениях - изучи API astro-rust, не придумывай формулы

Версии инструментов и библиотек (актуальные на август 2025)
Rust: 1.89+ (минимальная версия от 2025-06-26, compatible with stable channel)
Cargo edition: 2024

**Frontend Stack**:
Vite: 7.1.1 (latest stable, major upgrade from 7.x)
React: 19.1.1 (latest stable, major upgrade from 19.x)
TypeScript: 5.9.2 (latest stable)
@vitejs/plugin-react: 4.7.0 (latest stable)
Babylon.js: 8 (major pin; runtime uses latest 8.x, e.g., 8.22.x at time of build)
@babylonjs/core: 8 (major pin)
@babylonjs/materials: 8 (major pin)
@babylonjs/loaders: 8 (major pin)
@babylonjs/gui: 8 (major pin)

**Backend Stack**:
Axum: 0.8.4 (latest stable)
Tokio: 1.47.0 (latest stable)
SQLX: 0.8.6 (latest stable - runtime-tokio-rustls, postgres, macros, offline)
Serde: 1.0.210 (latest stable)
jsonwebtoken: 9.3.0 (latest stable)
tower-http: 0.6.2 (latest stable)
tracing/tracing-subscriber: 0.3.19 (latest stable)
teloxide: 0.13.0 (latest stable)
time: 0.3.37 (latest stable)
uuid: 1.11.0 (latest stable)
anyhow: 1.0.94 / thiserror: 2.0.3 (latest stable)

**WASM Stack**:
wasm-bindgen: 0.2.99 (latest stable)
wasm-pack: 0.13.0 (latest stable)
js-sys: 0.3.76 / web-sys: 0.3.76 (latest stable)
vite-plugin-wasm: 3.5.0 (latest stable)
vite-plugin-top-level-await: 1.6.0 (latest stable)

**Fullstack**:
dioxus: 0.7.0-alpha.3 (ALPHA - cutting-edge, completely rewritten)
config: 0.14.1 / figment: 0.10.21 (latest stable)

Принципы производительности и O(1)

Горячий путь кадра: ровно один вызов WASM compute_state(t) на кадр; доступ к результатам через view на WebAssembly.Memory (Float64Array) без копирования; O(1) доступ к значениям.
В Babylon.js: ни одной аллокации в кадре — переиспользование Vector3/Quaternion; обновления через методы ToRef/copyFromFloats.
В бэкенде: пути аутентификации и WS-авторизации — O(1) по времени обработки запроса с учётом кэширования Telegram статуса; асимптотика зависит от конкретных внешних I/O, но внутренние операции не вводят лишних аллокаций/копий.
SQL: индексные планы, целевые SELECT по первичным/уникальным ключам, подготовленные запросы; использовать SQLX compile-time проверки; строгое ограничение N+1; транзакции минимальной длительности.
WS протокол: компактные сообщения, без избыточных полей; JSON для человекочитаемости, бинарные форматы (CBOR) при росте нагрузки.
Константные структуры данных на горячем пути, отказ от динамической рефлексии, ранние проверки.
Domain (пример)

Value объекты: JulianDay, EclipticCoords, CartesianCoords, TelegramUserId, UserId, JwtClaims.
Инварианты: валидатор диапазона долгот/широт, проверка систем отсчёта (ecliptic J2000), политика доступа (is_subscribed ∧ role ≥ …).
Без зависимостей на инфраструктуру.
Rust пример (domain/types.rs):
pub struct JulianDay(pub f64);

pub struct EclipticSpherical {
pub lon_rad: f64,
pub lat_rad: f64,
pub r_au: f64,
}

pub struct Cartesian {
pub x: f64,
pub y: f64,
pub z: f64,
}

impl Cartesian {
#[inline]
pub fn from_ecliptic_spherical(s: &EclipticSpherical) -> Self {
// Прямое преобразование без аллокаций
let clat = s.lat_rad.cos();
let x = s.r_au * clat * s.lon_rad.cos();
let y = s.r_au * clat * s.lon_rad.sin();
let z = s.r_au * s.lat_rad.sin();
Self { x, y, z }
}
}

UseCases (пример портов)

AuthUseCase: login(username, password) -> JwtPair; refresh(token) -> JwtPair
TelegramLinkUseCase: issue_link_token(user_id) -> Token; confirm_link(token, telegram_id)
SubscriptionCheckUseCase: is_subscribed(telegram_user_id) -> bool (caching)
ProfileUseCase: get_profile(user_id), update_profile(…)
В портах Repository/ClientTraits: UsersRepo, TelegramApi, Cache (Redis/Memory), TokenService.
Пример trait:
#[async_trait::async_trait]
pub trait TelegramApi {
async fn is_member_of_channel(&self, user_id: i64) -> anyhow::Result<bool>;
}

WASM модуль (wasm-astro/)
Требования:

Rust 1.89+, no_std не требуется (web-target), но без лишних аллокаций в горячем пути.
Только числовые данные через общий буфер; никакой сериализации/JSON в кадре.
Один предвыделенный буфер с фиксированной раскладкой.
Экспорт указателя и длины; JS создаёт Float64Array(memory.buffer, ptr, len).
**🌟 АСТРОНОМИЧЕСКАЯ СИСТЕМА КООРДИНАТ (КРИТИЧЕСКИ ВАЖНО):**
- **ГЕЛИОЦЕНТРИЧЕСКАЯ МОДЕЛЬ ДЛЯ ПЛАНЕТ**: Солнце в центре сцены (0,0,0), планеты (включая Землю) вращаются вокруг Солнца
- **ГЕОЦЕНТРИЧЕСКАЯ МОДЕЛЬ ДЛЯ ЛУНЫ**: Луна позиционируется относительно позиции Земли (Луна вращается вокруг Земли)
- **WASM возвращает**: Декартовы координаты (x,y,z) всех небесных тел в едином буфере с правильными моделями координат
Пример (упрощённый, без ошибок и с заглушками astro-rust):
// Cargo.toml
// [lib]
// crate-type = ["cdylib"]
// [dependencies]
// wasm-bindgen = latest stable
// once_cell = latest stable
// # astro = "2.0.0" // saurvs/astro-rust

use wasm_bindgen::prelude::*;
use std::cell::RefCell;

const OUT_LEN: usize = 9; // [sun_x,sun_y,sun_z, earth_x,earth_y,earth_z, moon_x,moon_y,moon_z]

thread_local! {
static OUT_BUF: RefCell<[f64; OUT_LEN]> = RefCell::new([0.0; OUT_LEN]);
}

#[wasm_bindgen]
pub fn out_len() -> usize { OUT_LEN }

#[wasm_bindgen]
// Используем только compute_state; compute_all удалён
OUT_BUF.with(|b| {
let mut buf = b.borrow_mut();
// TODO: использовать исключительно astro-rust библиотеку из ./astro-rust/ (локальная копия с багфиксами) : VSOP87 + ELP-2000/82; учесть ΔT/TT
// 🚨 ВАЖНО: НЕ ИЗМЕНЯТЬ код в папке astro-rust/ - это неприкосновенный код библиотеки!
// 
// 🌟 АСТРОНОМИЧЕСКИЕ МОДЕЛИ:
// ГЕЛИОЦЕНТРИЧЕСКАЯ: Солнце в центре (0,0,0), планеты используют VSOP87 гелиоцентрические координаты
// ГЕОЦЕНТРИЧЕСКАЯ: Луна использует ELP-2000/82 геоцентрические координаты относительно Земли
//
// Здесь демонстрация с заглушками:
// sun_xyz := (0,0,0) - Солнце в центре гелиоцентрической сцены
buf[0] = 0.0; buf[1] = 0.0; buf[2] = 0.0;
// earth_xyz := позиция Земли от Солнца (гелиоцентрическая VSOP87)
let (ex, ey, ez) = fake_earth_heliocentric_xyz(jd);
buf[3] = ex; buf[4] = ey; buf[5] = ez;
// moon_xyz := позиция Луны от Земли (геоцентрическая ELP-2000/82) + смещение к позиции Земли
let (mx, my, mz) = fake_moon_geocentric_xyz(jd);
buf[6] = ex + mx; buf[7] = ey + my; buf[8] = ez + mz;
buf.as_ptr()
})
}

fn fake_earth_heliocentric_xyz(jd: f64) -> (f64,f64,f64) {
// Заглушка для примера: Земля на орбите вокруг Солнца (VSOP87)
let t = (jd - 2451545.0) / 365.25; // годы от J2000
let earth_orbit_radius = 1.0; // 1 AU
let earth_orbital_angle = t * 2.0 * std::f64::consts::PI; // полный оборот за год
(earth_orbit_radius * earth_orbital_angle.cos(), 
 earth_orbit_radius * earth_orbital_angle.sin(), 
 0.0)
}

fn fake_moon_geocentric_xyz(jd: f64) -> (f64,f64,f64) {
// Заглушка для примера: Луна вокруг Земли (ELP-2000/82 геоцентрические координаты)
let t = (jd - 2451545.0) / 29.53; // лунные месяцы от J2000
let moon_orbit_radius = 0.00257; // ~384400 км в AU
let moon_orbital_angle = t * 2.0 * std::f64::consts::PI; // полный оборот за месяц
(moon_orbit_radius * moon_orbital_angle.cos(), 
 moon_orbit_radius * moon_orbital_angle.sin(), 
 0.001)
}

Важно: реальная интеграция с saurvs/astro-rust, корректные преобразования времени (UTC→TT/TDB), и минимизация тригонометрии в JS.

Frontend (frontend/, Vite + TS + Babylon.js)

TS строгий (noImplicitAny, strictNullChecks).
Импорт WASM как ESM. Использовать vite-plugin-wasm/top-level-await.
Рендер‑цикл: один вызов compute_state(jd), чтение Float64Array(memory.buffer, ptr, 14) и переиспользование view без аллокаций.
Babylon.js: переиспользование объектов; freeze для статичных.
Пример TS:
import { Engine, Scene, ArcRotateCamera, HemisphericLight, MeshBuilder, Vector3 } from "@babylonjs/core";
import init, { compute_state, out_len, memory } from "../wasm-astro/pkg/wasm_astro.js";

async function main() {
await init();
const canvas = document.createElement("canvas");
Object.assign(canvas.style, { width: "100%", height: "100%", position: "fixed", inset: "0" });
document.body.appendChild(canvas);

const engine = new Engine(canvas, true, { preserveDrawingBuffer: false, stencil: false });
const scene = new Scene(engine);
// ✅ Координаты: используем левостороннюю систему Babylon по умолчанию.
// Совместимость с астрономическими данными обеспечивается на слое сцены (единственный RH→LH Z‑flip при присвоении позиций)
const camera = new ArcRotateCamera("cam", Math.PI/2, Math.PI/3, 3, Vector3.Zero(), scene);
camera.attachControl(canvas, true);
new HemisphericLight("h", new Vector3(0,1,0), scene);

const earth = MeshBuilder.CreateSphere("earth", { diameter: 0.1 }, scene);
const moon = MeshBuilder.CreateSphere("moon", { diameter: 0.027 }, scene);

const outLen = out_len();
let outView: Float64Array;

const posMoon = new Vector3(0,0,0);

scene.onBeforeRenderObservable.add(() => {
const now = performance.now();
const jd = toJulianDay(now); // реализовать корректно
const ptr = compute_state(jd);
if (!outView) outView = new Float64Array((memory as WebAssembly.Memory).buffer, ptr, outLen);


// Buffer: [sun_x,sun_y,sun_z, moon_x,moon_y,moon_z, mercury_x,mercury_y,mercury_z, venus_x,venus_y,venus_z, earth_x,earth_y,earth_z, ...]
// ✅ ГЕЛИОЦЕНТРИЧЕСКАЯ МОДЕЛЬ: Солнце всегда в центре (0,0,0)
const posSun = Vector3.Zero(); // Солнце в центре
sun.position.copyFrom(posSun);

// ✅ ГЕЛИОЦЕНТРИЧЕСКАЯ МОДЕЛЬ: Земля использует реальные гелиоцентрические координаты (индексы 12-14)
const posEarth = new Vector3(outView[12] * 20, outView[13] * 20, outView[14] * 20); // масштаб 20 AU для видимости
earth.position.copyFrom(posEarth);

// ✅ ГЕОЦЕНТРИЧЕСКАЯ МОДЕЛЬ: Луна = позиция Земли + геоцентрический оффсет (уже рассчитано в WASM)
posMoon.copyFromFloats((outView[12] + outView[3]) * 20, (outView[13] + outView[4]) * 20, (outView[14] + outView[5]) * 20); 
moon.position.copyFrom(posMoon);
});

engine.runRenderLoop(() => scene.render());
}

function toJulianDay(ms: number): number {
// Упростим для примера. В проде — UTC→JD→TT/TDB с ΔT
const jd1970 = 2440587.5; // JD эпоха Unix
return jd1970 + ms / 86400000.0;
}

main().catch(console.error);

GUI

Основной UI (кнопки “Войти/Профиль”, статусы) — HTML/CSS overlay поверх canvas. Это быстрее, гибче, проще для доступности.
3D‑привязанный UI (метки) — Babylon GUI точечно.
VR-ready: в будущем world-space панели для VR, а HTML скрывать в XR.
Бэкенд (backend/, Axum)

Axum маршруты: REST API, WS, статика. Чистая архитектура: хэндлеры зависят от use-cases через DI (Arc<AppContainer>).
JWT: RS256 предпочтительно (ротация ключей через JWKS, при необходимости). Минимальные клеймы: sub, exp, iat; кастомный is_telegram_subscribed.
WebSocket: клиент отправляет JWT первым сообщением; сервер валидирует и привязывает пользовательский контекст.
Пример структуры:
use axum::{Router, routing::{get, post}, extract::State, response::IntoResponse};
use std::sync::Arc;

#[derive(Clone)]
struct AppContainer {
auth_uc: Arc<dyn AuthUseCase>,
// ...
}

async fn login(State(app): State<Arc<AppContainer>>, payload: LoginDto) -> impl IntoResponse {
// валидируем DTO, вызываем use-case, возвращаем токены
}

fn router(app: Arc<AppContainer>) -> Router {
Router::new()
.route("/api/auth/login", post(login))
.merge(static_routes())
.with_state(app)
}

JWT (пример сервисного кода):
pub struct JwtService {
enc: jsonwebtoken::EncodingKey,
dec: jsonwebtoken::DecodingKey,
alg: jsonwebtoken::Algorithm,
}

impl JwtService {
pub fn encode(&self, claims: &Claims) -> anyhow::Result<String> {
use jsonwebtoken::{Header, encode};
let mut h = Header::new(self.alg);
// kid, typ при необходимости
Ok(encode(&h, claims, &self.enc)?)
}
pub fn decode(&self, token: &str) -> anyhow::Result<Claims> {
use jsonwebtoken::{Validation, decode};
let data = decode::<Claims>(token, &self.dec, &Validation::new(self.alg))?;
Ok(data.claims)
}
}

SQLX и PostgreSQL

Подключение через sqlx::Pool<Postgres>, tls = rustls.
Миграции (sqlx migrate). Включить sqlx offline feature и макросы для compile-time проверки запросов.
Схема:
users(id uuid pk, username citext unique, password_hash text, telegram_user_id bigint null, created_at timestamptz, updated_at timestamptz)
refresh_tokens(id uuid pk, user_id, token_hash, exp)
telegram_linking(token uuid pk, user_id, expires_at timestamptz)
Индексы по username, telegram_user_id, exp.
Пароли — Argon2id (memory-hard), pepper + per-user salt.
Telegram интеграция

teloxide для бота. Флоу:
dioxus-app вызывает backend: issue_link_token -> записываем в telegram_linking.
пользователь отправляет токен боту.
бот вызывает backend endpoint confirm_link(token, telegram_user_id).
backend проверяет getChatMember и сохраняет user.telegram_user_id, кэширует is_subscribed.
Кэш: Redis или in-memory с TTL. Пример интерфейса Cache: get/set с serde_json для значений.
WebSockets

/ws/user: upgrade → ожидать первое сообщение {type:"auth", token:"..."} → валидация → присвоить UserContext → обмен сообщений.
Формат сообщений: компактный JSON (или CBOR).
Реконнект с бэкофф на фронте.
Dioxus (dioxus-app/) - Полноценный личный кабинет

Маршруты: 
- /cabinet/auth/login, /cabinet/auth/register - авторизация через Telegram
- /cabinet/profile - профиль пользователя, настройки языка
- /cabinet/learning - обучающие материалы, курсы по астрологии
- /cabinet/subscription - управление подпиской на Telegram канал
- /cabinet/admin - админка для управления пользователями
Server functions: типобезопасные вызовы к backend.
После логина — редирект на "/" (frontend сцена) с обновленным статусом.
Хранение токенов: access — в памяти (внутри вкладки), refresh — HttpOnly cookie (опционально).
Безопасность

HTTPS везде с принудительным редиректом; HSTS; TLS 1.2+ с Let's Encrypt сертификатами.
Security headers: X-Frame-Options, X-Content-Type-Options, Referrer-Policy, HSTS.
CSP: default-src 'self'; connect-src 'self' wss: https:; script-src 'self' 'wasm-unsafe-eval' (для WASM).
SSL/TLS: автоматическое обновление сертификатов через certbot cron.
CSRF: не применимо для pure API с JWT и SameSite=strict; если формы — CSRF токен.
Пароли: Argon2id, ограничение попыток (rate limiting/IP throttling).
JWT время жизни access короткое (например, 15 мин), refresh — дольше (ротация, revocation list).
WebSocket: wss:// в production, проверка X-Forwarded-Proto заголовка.
Логи: tracing + разноуровневые spans; PII не логировать.
Наблюдаемость и стабильность

tracing/tracing-subscriber с JSON форматтером; корелляция запросов (request-id).
метрики Prometheus (opentelemetry-плагины при необходимости).
Health endpoints /healthz, /readyz.
Анти‑backpressure: ограничение числа одновременных WS, таймауты, keep-alive, ping/pong.
Сборка и деплой

### Production Architecture (nginx + Axum)
```
/opt/starscalendars/backend          ← Axum сервер (localhost:8080)
/var/www/starscalendars/             ← Статические файлы (nginx)
├── index.html                       ← React 3D сцена
├── assets/main-abc.js               ← Скомпилированный фронтенд
├── textures/                        ← Текстуры для Babylon.js
│   ├── earth.jpg, moon.jpg, sun.jpg
│   ├── stars/milky-way.jpg
│   └── planets/*.jpg
├── models/                          ← 3D модели (.babylon, .glb)
├── wasm-astro/                      ← WASM астрономические расчеты
│   ├── starscalendars_wasm_astro.js
│   └── starscalendars_wasm_astro_bg.wasm
└── cabinet/                         ← Dioxus полноценное приложение
    ├── index.html                   ← Личный кабинет пользователя
    ├── auth/                        ← Авторизация через Telegram
    ├── profile/                     ← Профиль и настройки  
    ├── learning/                    ← Обучающие материалы
    ├── subscription/                ← Управление подпиской
    ├── admin/                       ← Админка (для админов)
    └── assets/
```

### AlmaLinux 9.4 Server Setup
```bash
# Установка системных зависимостей  
sudo dnf install -y gcc openssl-devel postgresql-devel nginx rust cargo

# Создание директорий
sudo mkdir -p /opt/starscalendars
sudo mkdir -p /var/www/starscalendars

# Компиляция сервера на продакшн машине
cargo build --release --target-cpu=native
sudo cp target/release/backend /opt/starscalendars/

# Копирование статических файлов фронтенда
rsync -av frontend/dist/ /var/www/starscalendars/
rsync -av dioxus-app/dist/ /var/www/starscalendars/cabinet/
```

### Nginx Configuration (HTTPS + Certbot)
```nginx
# HTTP редирект на HTTPS
server {
    listen 80;
    server_name starscalendars.com www.starscalendars.com;
    return 301 https://$server_name$request_uri;
}

# HTTPS основной сервер
server {
    listen 443 ssl http2;
    server_name starscalendars.com www.starscalendars.com;
    
    # SSL сертификаты от Let's Encrypt (certbot)
    ssl_certificate /etc/letsencrypt/live/starscalendars.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/starscalendars.com/privkey.pem;
    include /etc/letsencrypt/options-ssl-nginx.conf;
    ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;
    
    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Frame-Options DENY always;
    add_header X-Content-Type-Options nosniff always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    
    # CSP для безопасности
    add_header Content-Security-Policy "default-src 'self'; connect-src 'self' wss: https:; img-src 'self' data: https:; font-src 'self' https:; style-src 'self' 'unsafe-inline' https:; script-src 'self' 'wasm-unsafe-eval' https:;" always;
    
    # Статика напрямую через nginx (производительность)
    location / {
        root /var/www/starscalendars;
        try_files $uri $uri/ /index.html;
        
        # Кэширование статических ресурсов
        location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|wasm)$ {
            expires 1y;
            add_header Cache-Control "public, immutable";
        }
    }
    
    # Личный кабинет Dioxus (авторизация, обучение, профиль, админка)
    location /cabinet/ {
        root /var/www/starscalendars;
        try_files $uri $uri/ /cabinet/index.html;
    }
    
    # API и WebSocket проксируются на Axum
    location /api/ {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
    
    location /ws/ {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
    }
}
```

### Let's Encrypt SSL Setup
```bash
# Установка certbot
sudo dnf install -y certbot python3-certbot-nginx

# Получение SSL сертификата
sudo certbot --nginx -d starscalendars.com -d www.starscalendars.com

# Автоматическое обновление сертификата
sudo crontab -e
# Добавить строку:
0 12 * * * /usr/bin/certbot renew --quiet

# Проверка автообновления
sudo certbot renew --dry-run
```

### systemd Service
```ini
# /etc/systemd/system/starscalendars.service
[Unit]
Description=StarsCalendars Backend
After=network.target postgresql.service

[Service]
Type=simple
ExecStart=/opt/starscalendars/backend
WorkingDirectory=/opt/starscalendars
Restart=always
RestartSec=10
Environment=DATABASE_URL=postgresql://user:pass@localhost/starscalendars

[Install]
WantedBy=multi-user.target
```

Vite build для frontend; wasm-pack build --release для wasm-astro; dioxus build.
**НЕ ИСПОЛЬЗУЕМ DOCKER**: Ручное развертывание на AlmaLinux 9.4 server. Frontend компилируется заранее, backend компилируется на продакшн сервере. nginx отдаёт статику, Axum только API/WebSocket.
CI: форматирование (rustfmt), clippy (deny(warnings) для критичного кода), cargo-audit, sqlx prepare.
VR-ready пометки

UI разделён: HTML для 2D, возможность world-space UI на Babylon GUI для XR без ломки архитектуры.
В будущем: WebXR session injection на клиенте, рендер тех же мешей; логин — вне XR или world-space минимальный.
Ссылки на актуальные практики

Axum/бэкенд производительность и архитектура — см. продвинутые разборы и гайды по оптимизациям в Rust бэкендах (medium.com, medium.com, medium.com). Хотя они рассматривают и другие фреймворки (Actix), подходы к низкой латентности, аллокациям и архитектурной дисциплине применимы и к Axum.
Продакшн шаблоны Axum/WebApp (структура проекта, слои, best practices): github.com.
Критерии приемки

FPS: стабильные 60 кадр/с на эталонном десктопе (указать конфигурацию).
На кадр: 1 вызов WASM; нулевая аллокация в рендер‑цикле (проверено профилировщиком).
Бэкенд: p95 latency < X ms для /auth/login и /ws handshake (указать SLA).
SQL: все запросы проходят compile-time проверку SQLX; миграции повторяемы; индексные планы подтверждены EXPLAIN ANALYZE.
Безопасность: прохождение статических анализов (clippy, cargo-audit), CSP работает, HTTPS, токены корректно выдаются/освежаются.
Кроссбраузерность: Chromium/Firefox/Safari последних релизов; мобильные Chrome/Safari.
VR-ready: экспериментальная ветка с прототипом WebXR запускается (в будущем), без изменений бэкенда.
## Многоязычная система (10 языков)

### Поддерживаемые языки (приоритетные уровни)
- **Tier 1** (приоритетные): Russian, English, Chinese, Spanish, Hindi
- **Tier 2** (400M+ носителей): Portuguese, German, French, Japanese  
- **Tier 3** (специализированные): Armenian

**ИЗМЕНЕНИЕ**: Исключены арабский и грузинский языки из поддержки. Система поддерживает 10 языков. Russian перенесен в первый приоритет.

### Архитектура i18n
- **Fluent** для локализации (ICU MessageFormat)
- **Кроссплатформенная синхронизация**: Dioxus ↔ Babylon.js ↔ HTML overlay
- **Культурные адаптации**: священные календари, региональные форматы
- **Производительность**: <200ms загрузка языка, <100ms переключение

### Реализация
```rust
// libs/domain/i18n.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    English, Chinese, Spanish, Hindi,
    Portuguese, German, French, Japanese,
    Russian, Armenian,
}

impl Language {
    pub fn icu_locale(&self) -> &'static str {
        match self {
            Language::English => "en-US",
            Language::Chinese => "zh-CN", 
            Language::Spanish => "es-ES",
            Language::Hindi => "hi-IN",
            // ... остальные языки
        }
    }
}
```

### GUI стратегия
- **HTML/CSS overlay** для основного UI (меню, кнопки, формы)
- **Babylon.js GUI** только для 3D-интегрированных элементов
- **Производительность**: HTML overlay значительно быстрее Babylon GUI
- **Локализация**: полная поддержка всех 10 языков

### Telegram интеграция
- **Многоязычный бот** с культурными адаптациями
- **Автоматическое определение языка** по Telegram настройкам
- **Локализованные команды** и ответы
- **Культурные символы** в сообщениях

## Дополнительные архитектурные детали

### WASM-JS интероперабельность
- **Float64Array view** на WebAssembly.Memory без копирования
- **Thread-local буферы** для нулевого копирования
- **Единый вызов** `compute_state(t)` на кадр
- **Избегание** передачи строк между WASM-JS

### Процесс привязки Telegram
- **Генерация UUID токенов** для безопасной привязки
- **API endpoints**: `/api/telegram/link-account`, `/api/telegram/confirm-link`
- **Валидация через getChatMember** для проверки подписки
- **Кэширование статуса** подписки для производительности

### WebSocket протокол
- **Компактный JSON** для человекочитаемости
- **CBOR** для бинарных данных при росте нагрузки
- **Первое сообщение**: JWT аутентификация
- **Сериализация через serde** для типобезопасности

### Dioxus Server Functions
- **Типобезопасный RPC** между клиентом и сервером
- **Компиляция в WASM** для производительности
- **Интеграция с Axum** через HTTP endpoints
- **Кэширование** переводов в памяти

### Процесс сборки
- **pnpm workspaces** для монорепозитория
- **Vite плагины**: `vite-plugin-wasm`, `vite-plugin-top-level-await`
- **wasm-pack** с флагом `--release` для оптимизации
- **Ручное развертывание** на AlmaLinux 9.4 без Docker контейнеров

## Примечания

Уточнить точные версии crate-ов в Cargo.toml на docs.rs.
Регулярные обновления через Dependabot/Renovate.
Проработать точные модели времени: ΔT таблица, преобразования UTC→TT/TDB; при необходимости — вынести в отдельный модуль libs/time.
При росте нагрузки на Telegram API — внедрить Redis кэш (TTL) и батч‑проверки.

## 📌 План ближайших работ (оптимизация горячего пути и визуальный lock Луны)
- Расширить `compute_state(jd)` так, чтобы в STATE возвращались предрасчитанные значения: `lunar_ra_rad`, `lunar_dec_rad`, `apparent_sidereal_time_rad`, `sublunar_lon_east_rad`, `sublunar_lat_rad`, а также Earth‑local единичный вектор Земля→Луна (или согласованный аналог)
- Это позволит полностью убрать тригонометрию из TypeScript и оставить ровно один wasm‑вызов на кадр
- Реализовать визуальный tidal lock Луны (одна сторона к Земле) на уровне сцены, используя Earth→Moon направление из STATE; либрации пока не моделируем
