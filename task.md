Title: StarsCalendars — Zodiac + Lunar Events (docs → WASM → frontend) (current)
Selected from tasks-list.md
# Title: StarsCalendars — Zodiac + Lunar Events (docs → WASM → frontend) (current)

## Goal
1) Полная синхронизация проектной документации (канон, без потери деталей из чата), включая **канон камеры Earth↔Moon** и старт сцены.  
2) Реализация **знаков зодиака (tropical/sidereal)** и **лунных событий** (phases/nodes/apsides/eclipses/void-of-course) в WASM + инфопанель у Луны на сцене.  
3) Временно: вместо постинга в TG — UI подсказка “РАСШИФРОВКА ДНЯ в @elioncalendar” в гуи сцены над квантовой датой.

## Hard rules (канон)
- Любые новые расчёты **всегда** опираются на вычисления `astro-rust` (локальная копия, read-only). Сначала ищем готовую функцию в `astro-rust/src/*`.
- Допускаются только **derived classifiers/search** для событий (например, eclipses/voc) поверх углов/узлов/расстояний, полученных через `astro-rust` вызовы. Не добавлять “новые эфемеридные формулы”.
- Горячий путь: ровно **1× `compute_state(jd)`** на кадр, zero-copy `Float64Array` view.

## Subtasks (этот цикл)
- [ ] Док-синхронизация: `tz.md`, `tasks-list.md`, `task.md`, `README.md`, `QUALITY.md`, `CLAUDE.md`, `.cursorrules`, `.claude/agents/*.md`, `docs/context-bootstrap.md`, `docs/wasm-astro-api-checklist.md`
- [ ] `plan.md`: вынести уникальное в канон → удалить `plan.md` (без второго источника правды)
- [ ] WASM: research-first список функций `astro-rust` для zodiac/lunar events (в `docs/wasm-astro-api-checklist.md`)
- [ ] WASM: расширить STATE, добавляя слоты **только в конец** (без изменения существующих индексов)
- [ ] WASM: zodiac (Sun+Moon) tropical/sidereal
- [ ] WASM: lunar events (phases/nodes/apsides/eclipses/voc) — off-frame helpers + необходимые текущие значения в STATE
- [ ] Frontend: LunarInfoPanel (Babylon GUI) у Луны, показывать только при `cameraTarget === 'moon'`
- [ ] Frontend: “РАСШИФРОВКА ДНЯ в @elioncalendar” внутри `#stats` (один overlay)

## Done Criteria
- Документация синхронизирована и содержит полный “канон камеры” и текущие алгоритмы сцены/WASM без противоречий.
- `plan.md` слит в канон и удалён.
- WASM и сцена показывают zodiac/lunar info у Луны; TG hint виден рядом с FPS; один overlay (`#stats`).
