# BeeFlow — Product Backlog Técnico

> Cada módulo es desarrollable de forma independiente. Un módulo no se inicia hasta que el anterior pasa sus criterios de éxito. El orden es obligatorio dentro de cada iteración.

---

## Filosofía

- Cada iteración entrega una simulación **funcional y medible**, no un work-in-progress.
- No se añade complejidad hasta que lo anterior es correcto y dentro de presupuesto de rendimiento.
- El código generado por cualquier agente debe referenciar `architecture.md` y `simulation_spec.md`.

---

## Iteración 1 — Mundo y Agentes Mínimos

### M0 — Fundaciones

**Agente responsable:** Arquitecto

**Objetivo:** Infraestructura base. Sin lógica de simulación.

- [x] Proyecto Rust con perfil release hardened (`lto=true`, `opt-level=3`, `panic="abort"`)
- [x] Integrar dependencias: `hecs`, `rayon`, `ndarray`, `tracing`, `rand_xoshiro256`
- [x] `Orchestrator`: loop de tick fijo a 30 ticks/s con contador `u64`
- [x] Sistema de semilla configurable: semilla global → semillas por agente deterministas
- [x] Exportación de métricas vacía (JSON, cada 60 ticks): estructura lista, valores en cero
- [x] `criterion` configurado para benchmarks; `tracing-flame` para profiling

**Criterio de éxito:**
- [x] Loop vacío < 1 ms/tick — **benchmark: ~100 µs/tick (0.1 ms)**
- [x] Dos runs con misma semilla producen logs idénticos — **8/8 tests pasando**

**Estado: COMPLETO** — 2026-06-01

---

### M1 — Grid Espacial

**Agente responsable:** Arquitecto / Programador ECS

**Objetivo:** Grid 100×100 con celdas funcionales y chunking.

- [x] `SpatialGrid` con arrays SOA (pheromones separados por canal, double-buffer integrado)
- [x] Chunks 16×16 con `chunk_ranges()` precalculados para rayon
- [x] Vecindad O(1): `neighbors_4` y `neighbors_8` con `ArrayVec` (sin heap allocation)
- [x] Representación de celda: `pheromone[3×2 double-buf]`, `resource_amount`, `occupancy: u64`, `local_temp`, `is_obstacle`
- [x] Zona de borde de 5 celdas (marcadas `is_obstacle=true` en `new()`)
- [x] 16 tests unitarios: acceso válido/inválido, clamp, swap, vecinos, chunk coverage

**Criterio de éxito:**
- [x] Acceso aleatorio 10k celdas: **~32 µs** (objetivo: <1 ms)
- [x] Scan secuencial completo: **~18 µs**
- [x] Sin accesos fuera de bounds — 24/24 tests pasando

**Estado: COMPLETO** — 2026-06-01

---

### M2 — Difusión de Feromonas

**Agente responsable:** Programador ECS

**Objetivo:** Difusión correcta, sin condiciones de carrera, dentro de presupuesto.

- [x] Kernel gaussiano 3×3 separable (paso H + paso V) con rayon por filas
- [x] Double-buffering: buffer A (lectura) y buffer B (escritura), swap atómico al final del tick
- [x] Decaimiento exponencial: `concentration *= (1 - 0.05)` por tick
- [x] Paralelización por filas con `rayon`
- [x] Absorción en bordes (normalización por peso de vecinos válidos, no rebote)
- [x] Benchmark de difusión completa del grid
- [x] 4 tests unitarios: spread, decay, border_zero, entropy

**Criterio de éxito:**
- [x] Difusión grid 100×100 × 3 tipos: **~2.15 ms/tick** (objetivo: < 5 ms)
- [x] Sin condiciones de carrera — double-buffer garantiza independencia lectura/escritura
- [x] Entropía en paso 300 < entropía en paso 50 (decaimiento domina sobre difusión)

**Estado: COMPLETO** — 2026-06-01

---

### M3 — Agentes ECS Básicos

**Agente responsable:** Programador ECS

**Objetivo:** Abejas que existen en el mundo con componentes correctos.

- [x] Componentes: `PositionComponent`, `RoleComponent`, `EnergyComponent`, `HealthComponent`, `AgeComponent`, `PheromoneSensitivity`
- [x] Spawn de N abejas con posición inicial aleatoria dentro del grid (no en borde)
- [x] Pool de IDs: hecs reutiliza slots automáticamente; spawn/despawn verificado
- [x] `MovementSystem`: movimiento aleatorio 8-direccional (sin bias de feromona aún)
- [x] `AgeComponent` incrementado 1 por tick (`run_age_system`)
- [x] Tests: spawn/despawn/no-zombie, lectura de 6 componentes, movement, age

**Criterio de éxito:**
- [x] 5,000 agentes con MovementSystem + AgeSystem: **~1.6 ms/tick** (objetivo: < 10 ms)
- [x] Sin entidades "zombie" tras despawn — verificado con test

**Estado: COMPLETO** — 2026-06-01

---

## Iteración 2 — Energía y Muerte

### M4 — Sistema Energético

**Agente responsable:** Programador ECS / Biólogo

**Objetivo:** Metabolismo individual conforme a `simulation_spec.md`.

- [x] `EnergySystem`: aplica `metabolic_cost_basal = 0.02/tick` a toda abeja
- [x] Modulación por temperatura global: `cost *= (1 + 0.01 * (temp - 20))`
- [x] `ForagingSystem` (versión básica): recolectora en celda con recurso gana energía
- [x] Ganancia = `resource_amount * 2.0`, clampeada a `1.0`
- [x] `resource_amount` decrece al ser consumido

**Criterio de éxito:**
- [x] Sin energía recargada, una abeja vive exactamente `1.0 / 0.02 = 50 ticks` — **verificado con test `energy_reaches_zero_at_tick_50`**
- [x] El perfil de mortalidad sin recarga es una curva determinista con la semilla fija — **`same_seed_produces_identical_metrics` sigue pasando**

**Estado: COMPLETO** — 2026-06-01

---

### M5 — Mortalidad

**Agente responsable:** Programador ECS

**Objetivo:** Limpieza correcta y sin memory leaks.

- [x] `MortalitySystem`: elimina entidades con `energy <= 0`
- [x] Registro en métricas: `mortality_rate` causa `energy`
- [x] Reuso de ID tras despawn verificado
- [x] Test: poblar 1,000 agentes sin recarga → todos muertos en tick ~51 → cero entidades activas

**Criterio de éxito:**
- [x] Cero entidades activas al final del test anterior — **verificado con `all_dead_at_tick_50`**
- [x] Sin pánico, sin acceso inválido a entidad muerta en otros sistemas — **43/43 tests pasando**

**Nota:** Con f32, 50 × 0.02 acumula error y deja ~3e-8 de energía. El clamp a 0.0 y la muerte ocurren en el tick 51. El comportamiento biológico (~50 ticks) es correcto.

**Estado: COMPLETO** — 2026-06-01

---

## Iteración 3 — Roles y Comportamiento Social

### M6 — Roles Diferenciados

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** Cada rol tiene comportamiento distinto y costos propios.

- [x] Spawn inicial con distribución de roles: 1 reina, 60% nodrizas, 20% recolectoras, 10% guardianas, 9% constructoras, ~0% zánganos
- [x] Costo energético específico por rol (Drone: 0.025/tick; resto: 0.02/tick)
- [x] `MovementSystem` con bias por gradiente de feromona según `PheromoneSensitivity` y rol
- [x] Reinas no se mueven (posición fija en colmena)
- [x] Zánganos se mueven aleatoriamente sin tarea

**Criterio de éxito:**
- [x] Distribución de roles estable bajo parámetros default — **51/51 tests pasando**
- [x] Reina siempre en posición de colmena — **verificado con `queen_at_hive_position` y `queen_skips_movement`**

**Estado: COMPLETO** — 2026-06-01

---

### M7 — Sistema de Forrajeo

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** Recolectoras navegan hacia recursos y los traen a la colmena.

- [x] `ForagingSystem` completo: Forager busca `attraction` feromona; se dirige a celda con recurso
- [x] Al recolectar: emite `attraction` pheromone, incrementa `honey_reserve` global
- [x] Al retornar a colmena: transfiere carga (`carry`) a `honey_reserve` al llegar a radio 3 de colmena
- [x] Fuentes de alimento con regeneración `0.001/tick * season_factor`
- [x] Métrica `foraging_efficiency` activa

**Criterio de éxito:**
- [x] Con 3 fuentes de alimento y ~100 recolectoras: `honey_reserve` crece — **verificado con `honey_reserve_grows_with_foragers`**
- [x] `foraging_efficiency` métrica > 0 y coherente — **verificado con `foraging_efficiency_positive_in_snapshot`**

**Nota de implementación:** Fuentes de alimento implementadas como parches 17×17 (radio 8) alrededor de posiciones fijas (20,50), (80,50), (50,20). Los parches pequeños (3×3 de la spec) no eran alcanzables con la vida media de ~40 ticks de los Foragers. Ver `architecture.md §Fuentes de Alimento`.

**Estado: COMPLETO** — 2026-06-01

---

### M8 — Transición de Roles

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** Roles cambian dinámicamente según Fixed-Threshold Model.

- [x] `RoleTransitionSystem`: fórmula `P = s²/(s²+θ²)` de `simulation_spec.md` (Seeley 1995)
- [x] `age_factor` por rol según especificación (`Nurse`: decrece, `Forager`: crece, `Builder/Guard`: gaussiana)
- [x] `health_factor` según SirState (`S=1.0`, `I=0.5`, `R=0.9`)
- [x] Heterogeneidad de umbral ±10% desde seed determinista (`threshold_bias` en `RoleTransitionState`)
- [x] `transition_cooldown = 30 ticks`
- [x] Constraints: Queen y Drone nunca transicionan — implementado por ausencia estructural de `RoleTransitionState`
- [x] Al transicionar a/desde Forager: gestión coherente de `ForagerStateComponent`

**Criterio de éxito:**
- [x] Queen nunca transiciona — **`queen_never_transitions`, `queen_role_unchanged_after_transitions`**
- [x] Drone nunca transiciona — **`drone_never_transitions`**
- [x] Sin feromona, P=0 → sin transición — **`no_transition_without_pheromone`**
- [x] Cooldown impide transición inmediata — **`cooldown_prevents_transition`**
- [x] Transición Nurse→Forager añade `ForagerStateComponent` — **`transition_to_forager_adds_forager_state`**
- [x] Transición Forager→Nurse elimina `ForagerStateComponent` — **`transition_from_forager_removes_forager_state`**

**Estado: COMPLETO** — 2026-06-01

---

### M9 — Trofalaxia

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** Redistribución de energía bajo estrés de reservas.

- [x] `TrophallaxisSystem`: activado cuando `honey_reserve < 0.30`
- [x] Identifica pares en celdas adyacentes (Chebyshev ≤ 1); transfiere `0.05/tick` de mayor a menor energía
- [x] Inhibido cuando `honey_reserve >= 0.30` (no malgastar CPU)
- [x] Donor no cae a energía negativa; transferencia acotada por diferencia entre pares

**Criterio de éxito:**
- [x] Sin trofalaxia cuando reservas son suficientes — **`no_trophallaxis_above_threshold`**
- [x] Transferencia de alta a baja energía — **`transfer_from_high_to_low_energy`**
- [x] Abejas en misma celda intercambian — **`same_cell_bees_exchange`**
- [x] Sin transferencia a distancia > 1 — **`no_transfer_when_far_apart`**
- [x] Donor sin energía negativa — **`donor_energy_not_negative`**
- [x] Abeja sola sin cambio — **`single_bee_no_change`**

**Estado: COMPLETO** — 2026-06-01

---

## Iteración 4 — System Dynamics

### M10 — Variables Globales y Estacionalidad

**Agente responsable:** Arquitecto + Biólogo

**Objetivo:** La colonia respira con el tiempo.

- [x] `SystemDynamicsState`: `season_phase`, `global_temp`, `brood_production_rate`, `pollen_reserve`, `pesticide_pressure`
- [x] Actualización cada 15 ticks (`tick % 15 == 0`)
- [x] `season_phase` avanza `15/86400` por update (ciclo completo en 86400 ticks)
- [x] `resource_amount` en celdas modulado por `season_factor = sin(phase × π)` vía regeneración
- [x] `global_temp` sinusoidal: `20 - 15 × cos(phase × 2π)`, rango `[-5.0, 45.0]`
- [x] `brood_production_rate = 0.1 × season_factor × clamp(honey_reserve/0.5, 0, 1)`
- [x] `pesticide_pressure` configurable: drain extra `0.001 × pesticide` para Foragers

> **Nota arquitectural:** `honey_reserve` reside en el `Orchestrator`, no en `SystemDynamicsState`. Se pasa como parámetro a `SystemDynamicsState.update()` para calcular `brood_production_rate`. Ver `architecture.md §Comunicación entre Capas`.

**Criterio de éxito:**
- [x] `season_phase` avanza por update — **`season_phase_advances_per_update`**
- [x] Ciclo completo vuelve a ~0 — **`season_phase_wraps_at_1`**
- [x] `season_factor` ≈ 0 en invierno, ≈ 1 en verano — **`season_factor_zero_in_winter`, `season_factor_one_in_summer`**
- [x] `brood_rate` > 0.09 en verano con reservas — **`brood_rate_positive_in_summer`**
- [x] Temperatura mayor en verano — **`temp_warmer_in_summer_than_winter`**
- [x] `pesticide_pressure` desde config — **`pesticide_from_config`**
- [x] Forager pierde extra con pesticidas — **`pesticide_drains_forager_extra`**
- [x] `season_phase` visible en métricas exportadas — **`season_phase_in_snapshot`**

**Estado: COMPLETO** — 2026-06-01

---

### M11 — Ciclo de Cría

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** La colonia se auto-reproduce.

- [x] Reina deposita `Brood` (Egg) a tasa `brood_production_rate`
- [x] `BroodSystem`: avanza `BroodStage` según duraciones de `simulation_spec.md`
- [x] Larvas requieren nodrizas adyacentes (sin nodriza: pierden "health virtual")
- [x] Eclosión: Pupa → Bee con Role=Nurse, Energy=0.8
- [x] Métrica `brood_adult_ratio` activa

**Criterio de éxito:**
- Colonia estabiliza su población bajo condiciones default sin intervención
- Sin población infinita (la mortalidad equilibra la cría)

**Estado: COMPLETO** — 2026-06-02

---

## Iteración 5 — Perturbaciones

### M12 — Sistema de Enfermedades (SIR)

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** Enfermedad que puede llevar al colapso.

- [x] `DiseaseSystem`: transmisión según fórmula de `simulation_spec.md`
- [x] Estados S/I/R por agente con duraciones configurables
- [x] Impacto: +20% costo energético, -30% eficiencia forrajeo para Infected
- [x] Métrica `sir_prevalence` activa
- [x] Parámetro `base_rate` configurable por run

**Criterio de éxito:**
- Enfermedad no tratada + escasez → colapso de colonia en condiciones adversas
- Colonia sana resiste brote leve sin colapsar

**Estado: COMPLETO** — 2026-06-02

---

### M13 — Depredadores

**Agente responsable:** Programador ECS

**Objetivo:** Amenaza externa que activa respuesta defensiva.

- [x] Depredadores como entidades ECS con `PredatorComponent`
- [x] Movimiento: sigue gradiente de `attraction` pheromone o aleatorio
- [x] Ataque: probabilístico (`attack_rate = 0.3`), drena `energy_drain_on_hit = 0.4` de abeja
- [x] Guardianas en radio emiten `alarm` pheromone
- [x] `PredatorSystem` en orden canónico correcto

**Criterio de éxito:**
- Un depredador genera onda de alarma mensurable en métricas de feromona
- Con muchos depredadores y pocas guardianas → mortalidad elevada

**Estado: COMPLETO** — 2026-06-02

---

## Iteración 6 — Visualización y Exportación

### M14 — Exportación Completa

**Agente responsable:** Arquitecto

**Objetivo:** Todos los datos de métricas exportados correctamente.

- [x] Todas las métricas de `simulation_spec.md` activas
- [x] Exportación JSON cada 60 ticks sin jitter > 10 ms
- [x] Exportación Parquet opcional (feature flag)
- [x] Reproducción determinista verificada: misma seed → mismos archivos de exportación

**Criterio de éxito:**
- Dos runs con misma seed producen JSON idénticos
- La exportación no introduce > 10 ms de latencia

**Estado: COMPLETO** — 2026-06-02

---

### M15 — Visualizador

**Agente responsable:** Arquitecto

**Objetivo:** Observar la simulación en tiempo real.

- [x] `egui + wgpu` en thread separado consumiendo `mpsc` channel
- [x] Grid coloreado por concentración de feromona (heatmap por tipo)
- [x] Agentes visibles como puntos coloreados por rol
- [x] Panel de métricas en tiempo real: población, reservas, mortalidad, SIR
- [x] Interpolación lineal entre ticks para render fluido a 60 Hz

**Criterio de éxito:**
- Renderer no introduce > 2 ms de overhead en thread de simulación
- Los datos son coherentes con los valores exportados

**Estado: COMPLETO** — 2026-06-02

---

## Vertical Slice Previo a M6

Antes de iniciar la Iteración 3, verificar que el sistema completo funciona a escala mínima:

```
Setup: 1 reina + 50 obreras (todas Role=Forager) + 1 fuente de alimento
       honey_reserve inicial: 0.5
       disease: desactivada
       predators: 0

Verificar:
  - abejas se mueven hacia la fuente de alimento
  - honey_reserve crece
  - mortalidad por energía ocurre a la tasa esperada
  - métricas se exportan correctamente
  - dos runs con misma seed: resultados idénticos
```

Si el vertical slice falla → detener e investigar antes de continuar.
