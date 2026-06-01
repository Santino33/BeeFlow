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

- [ ] Kernel gaussiano 3×3 separable (paso H + paso V) con `ndarray`
- [ ] Double-buffering: buffer A (lectura) y buffer B (escritura), swap atómico al final del tick
- [ ] Decaimiento exponencial: `concentration *= (1 - 0.05)` por tick
- [ ] Paralelización por chunks con `rayon`
- [ ] Absorción en bordes (no rebote)
- [ ] Benchmark de difusión completa del grid

**Criterio de éxito:**
- Difusión grid 100×100 × 3 tipos < 5 ms/tick en 8 cores
- Sin condiciones de carrera (verificar con ThreadSanitizer)
- La entropía espacial de una fuente puntual decrece monotónicamente tras eliminarla

---

### M3 — Agentes ECS Básicos

**Agente responsable:** Programador ECS

**Objetivo:** Abejas que existen en el mundo con componentes correctos.

- [ ] Componentes: `PositionComponent`, `RoleComponent`, `EnergyComponent`, `HealthComponent`, `AgeComponent`, `PheromoneSensitivity`
- [ ] Spawn de N abejas con posición inicial aleatoria dentro del grid (no en borde)
- [ ] Pool de IDs con reuso (evitar fragmentación de `hecs`)
- [ ] `MovementSystem`: movimiento aleatorio (sin bias de feromona aún)
- [ ] `AgeComponent` incrementado 1 por tick
- [ ] Tests: spawn/despawn, lectura de componentes, iteración por archetype

**Criterio de éxito:**
- 5,000 agentes con solo MovementSystem y AgeSystem: < 10 ms/tick
- Sin entidades "zombie" tras despawn

---

## Iteración 2 — Energía y Muerte

### M4 — Sistema Energético

**Agente responsable:** Programador ECS / Biólogo

**Objetivo:** Metabolismo individual conforme a `simulation_spec.md`.

- [ ] `EnergySystem`: aplica `metabolic_cost_basal = 0.02/tick` a toda abeja
- [ ] Modulación por temperatura global: `cost *= (1 + 0.01 * (temp - 20))`
- [ ] `ForagingSystem` (versión básica): recolectora en celda con recurso gana energía
- [ ] Ganancia = `resource_amount * 2.0`, clampeada a `1.0`
- [ ] `resource_amount` decrece al ser consumido

**Criterio de éxito:**
- Sin energía recargada, una abeja vive exactamente `1.0 / 0.02 = 50 ticks`
- El perfil de mortalidad sin recarga es una curva determinista con la semilla fija

---

### M5 — Mortalidad

**Agente responsable:** Programador ECS

**Objetivo:** Limpieza correcta y sin memory leaks.

- [ ] `MortalitySystem`: elimina entidades con `energy <= 0`
- [ ] Registro en métricas: `mortality_rate` causa `energy`
- [ ] Reuso de ID tras despawn verificado
- [ ] Test: poblar 1,000 agentes sin recarga → todos muertos en tick ~50 → cero entidades activas

**Criterio de éxito:**
- Cero entidades activas al final del test anterior
- Sin pánico, sin acceso inválido a entidad muerta en otros sistemas

---

## Iteración 3 — Roles y Comportamiento Social

### M6 — Roles Diferenciados

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** Cada rol tiene comportamiento distinto y costos propios.

- [ ] Spawn inicial con distribución de roles: 1 reina, 60% nodrizas, 20% recolectoras, 10% guardianas, 9% constructoras, ~0% zánganos
- [ ] Costo energético específico por rol (ver `simulation_spec.md`)
- [ ] `MovementSystem` con bias por gradiente de feromona según `PheromoneSensitivity` y rol
- [ ] Reinas no se mueven (posición fija en colmena)
- [ ] Zánganos se mueven aleatoriamente sin tarea

**Criterio de éxito:**
- Distribución de roles estable bajo parámetros default
- Reina siempre en posición de colmena

---

### M7 — Sistema de Forrajeo

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** Recolectoras navegan hacia recursos y los traen a la colmena.

- [ ] `ForagingSystem` completo: Forager busca `attraction` feromona; se dirige a celda con recurso
- [ ] Al recolectar: emite `attraction` pheromone, incrementa `honey_reserve` global
- [ ] Al retornar a colmena: transfiere energía a `honey_reserve`
- [ ] Fuentes de alimento con regeneración `0.001/tick * season_factor`
- [ ] Métrica `foraging_efficiency` activa

**Criterio de éxito:**
- Con 3 fuentes de alimento y 100 recolectoras: `honey_reserve` crece en condiciones default
- `foraging_efficiency` métrica > 0 y coherente

---

### M8 — Transición de Roles

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** Roles cambian dinámicamente según Fixed-Threshold Model.

- [ ] `RoleTransitionSystem`: implementar fórmula `P = s²/(s²+θ²)` de `simulation_spec.md`
- [ ] `age_factor` por rol según especificación
- [ ] `health_factor` según SirState
- [ ] Heterogeneidad de umbral ±10% desde seed determinista
- [ ] `transition_cooldown = 30 ticks`
- [ ] Constraints: Queen y Drone nunca transicionan

**Criterio de éxito:**
- La distribución de roles evoluciona sin intervención hacia proporciones biológicamente razonables
- El sistema no colapsa a un único rol uniforme

---

### M9 — Trofalaxia

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** Redistribución de energía bajo estrés de reservas.

- [ ] `TrophallaxisSystem`: activado cuando `colony_reserve < 0.30`
- [ ] Identifica pares en celdas adyacentes; transfiere `0.05/tick` de mayor a menor energía
- [ ] Inhibido cuando `colony_reserve >= 0.30` (no malgastar CPU)

**Criterio de éxito:**
- Con reservas bajas, la distribución de energía se homogeneiza entre agentes cercanos
- Sin trofalaxia cuando reservas son suficientes

---

## Iteración 4 — System Dynamics

### M10 — Variables Globales y Estacionalidad

**Agente responsable:** Arquitecto + Biólogo

**Objetivo:** La colonia respira con el tiempo.

- [ ] Implementar `SystemDynamicsState`: `honey_reserve`, `pollen_reserve`, `global_temp`, `season_phase`, `brood_production_rate`, `pesticide_pressure`
- [ ] Actualización cada 15 ticks
- [ ] `season_phase` avanza `1/86400` por tick (ciclo ~1 día real = 1 año simulado)
- [ ] `resource_amount` en celdas modulado por `season_phase`
- [ ] Retroalimentaciones conforme a `simulation_spec.md`

**Criterio de éxito:**
- La colonia crece en primavera/verano y decrece en otoño/invierno
- Las reservas muestran ciclos coherentes con la estacionalidad

---

### M11 — Ciclo de Cría

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** La colonia se auto-reproduce.

- [ ] Reina deposita `Brood` (Egg) a tasa `brood_production_rate`
- [ ] `BroodSystem`: avanza `BroodStage` según duraciones de `simulation_spec.md`
- [ ] Larvas requieren nodrizas adyacentes (sin nodriza: pierden "health virtual")
- [ ] Eclosión: Pupa → Bee con Role=Nurse, Energy=0.8
- [ ] Métrica `brood_adult_ratio` activa

**Criterio de éxito:**
- Colonia estabiliza su población bajo condiciones default sin intervención
- Sin población infinita (la mortalidad equilibra la cría)

---

## Iteración 5 — Perturbaciones

### M12 — Sistema de Enfermedades (SIR)

**Agente responsable:** Biólogo + Programador ECS

**Objetivo:** Enfermedad que puede llevar al colapso.

- [ ] `DiseaseSystem`: transmisión según fórmula de `simulation_spec.md`
- [ ] Estados S/I/R por agente con duraciones configurables
- [ ] Impacto: +20% costo energético, -30% eficiencia forrajeo para Infected
- [ ] Métrica `sir_prevalence` activa
- [ ] Parámetro `base_rate` configurable por run

**Criterio de éxito:**
- Enfermedad no tratada + escasez → colapso de colonia en condiciones adversas
- Colonia sana resiste brote leve sin colapsar

---

### M13 — Depredadores

**Agente responsable:** Programador ECS

**Objetivo:** Amenaza externa que activa respuesta defensiva.

- [ ] Depredadores como entidades ECS con `PredatorComponent`
- [ ] Movimiento: sigue gradiente de `attraction` pheromone o aleatorio
- [ ] Ataque: probabilístico (`attack_rate = 0.3`), drena `energy_drain_on_hit = 0.4` de abeja
- [ ] Guardianas en radio emiten `alarm` pheromone
- [ ] `PredatorSystem` en orden canónico correcto

**Criterio de éxito:**
- Un depredador genera onda de alarma mensurable en métricas de feromona
- Con muchos depredadores y pocas guardianas → mortalidad elevada

---

## Iteración 6 — Visualización y Exportación

### M14 — Exportación Completa

**Agente responsable:** Arquitecto

**Objetivo:** Todos los datos de métricas exportados correctamente.

- [ ] Todas las métricas de `simulation_spec.md` activas
- [ ] Exportación JSON cada 60 ticks sin jitter > 10 ms
- [ ] Exportación Parquet opcional (feature flag)
- [ ] Reproducción determinista verificada: misma seed → mismos archivos de exportación

**Criterio de éxito:**
- Dos runs con misma seed producen JSON idénticos
- La exportación no introduce > 10 ms de latencia

---

### M15 — Visualizador

**Agente responsable:** Arquitecto

**Objetivo:** Observar la simulación en tiempo real.

- [ ] `egui + wgpu` en thread separado consumiendo `mpsc` channel
- [ ] Grid coloreado por concentración de feromona (heatmap por tipo)
- [ ] Agentes visibles como puntos coloreados por rol
- [ ] Panel de métricas en tiempo real: población, reservas, mortalidad, SIR
- [ ] Interpolación lineal entre ticks para render fluido a 60 Hz

**Criterio de éxito:**
- Renderer no introduce > 2 ms de overhead en thread de simulación
- Los datos son coherentes con los valores exportados

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
