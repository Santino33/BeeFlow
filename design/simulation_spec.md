# BeeFlow — Especificación de Simulación (Source of Truth)

> Este documento es la **verdad biológica** del proyecto. Ningún agente puede inventar reglas, parámetros o comportamientos que no estén aquí. Toda modificación de mecánicas debe actualizar primero este archivo.

---

## Entidades

### Bee

```yaml
Bee:
  description: Abeja adulta activa (cualquier rol)
  components:
    - PositionComponent: (u16, u16)          # celda actual
    - RoleComponent: enum Role               # ver tabla de roles
    - EnergyComponent: f32                   # rango [0.0, 1.0]
    - HealthComponent: enum SirState         # S / I / R
    - AgeComponent: u32                      # ticks desde eclosión
    - PheromoneSensitivity: [f32; 3]         # umbral por tipo [alarm, task, attraction]
  invariants:
    - energy >= 0.0
    - energy <= 1.0
    - age >= 0
  death_condition: energy <= 0.0
```

### Brood

```yaml
Brood:
  description: Cría en desarrollo (pasiva, no se mueve)
  components:
    - PositionComponent: (u16, u16)
    - AgeComponent: u32                      # ticks desde oviposición
    - BroodStageComponent: enum BroodStage   # Egg / Larva / Pupa
  stage_durations:
    Egg:   36 ticks    # ~36 segundos simulados
    Larva: 144 ticks
    Pupa:  120 ticks
  on_pupa_complete: spawn Bee con Role=Nurse, Energy=0.8
```

### Predator

```yaml
Predator:
  components:
    - PositionComponent: (u16, u16)
    - EnergyComponent: f32
    - PredatorComponent:
        attack_rate: 0.3           # probabilidad de éxito por contacto/tick
        detection_radius: 2        # celdas
        energy_drain_on_hit: 0.4   # energía que pierde la abeja atacada
```

---

## Roles

```yaml
Role:
  Queen:
    description: Única por colmena. Pone huevos. No forrajea ni defiende.
    egg_rate: brood_production_rate   # huevos/tick (variable de System Dynamics)
    transitions_to: none              # la reina nunca cambia de rol

  Nurse:
    description: Atiende la cría. Consume energía alimentando larvas.
    age_range: [0, 200]               # ticks — jóvenes preferentemente
    energy_cost_nursing: 0.005/tick   # por cada larva atendida en celda adyacente
    pheromone_response: task          # responde a feromona de tarea (emitida por cría)

  Builder:
    description: Produce cera, construye panales.
    age_range: [100, 400]
    energy_cost_building: 0.008/tick
    pheromone_response: task

  Guard:
    description: Defiende la entrada de la colmena.
    age_range: [200, 500]
    alarm_pheromone_emission: 0.2/tick    # cuando detecta depredador
    pheromone_response: alarm

  Forager:
    description: Sale al exterior a recolectar néctar/polen.
    age_range: [400, max_lifespan]
    energy_cost_move: 0.003/tick          # coste adicional por movimiento exterior
    forage_gain_range: [0.5, 2.0]         # según resource_amount de la celda
    attraction_pheromone_emission: 0.15/tick  # marca fuentes de alimento
    pheromone_response: attraction

  Drone:
    description: Zángano. Reproducción. No trabaja.
    energy_cost_basal: 0.025/tick         # mayor que obrera
    transitions_to: none
```

---

## Energía

```yaml
Energy:
  max: 1.0
  initial: 0.8                         # al eclosionar

  metabolic_cost_basal: 0.02/tick       # toda abeja, toda condición
  metabolic_cost_infected: +0.20        # multiplicador adicional si SirState=Infected
  metabolic_cost_temp_factor:           # global_temp modula el costo
    equation: cost *= (1 + 0.01 * (global_temp - 20.0))
    # a 20°C sin modificación; +1% por cada grado sobre 20

  forage_gain:
    min: 0.5
    max: 2.0
    formula: resource_amount * 2.0      # ganancia proporcional al recurso disponible

  trophallaxis:
    trigger: colony_reserve < 0.30
    rate: 0.05/tick                     # transferido de donante a receptor por tick
    condition: ambos agentes en celdas adyacentes o misma celda
    direction: de mayor energía a menor energía

  death_threshold: 0.0                  # muerte inmediata al llegar a 0
```

---

## Feromonas

```yaml
Pheromone:
  types:
    alarm:
      index: 0
      source: Guard bajo ataque, Bee atacada por Predator
      emission_rate: 0.2/tick
      effect: recluta Guards; activa comportamiento defensivo
      response_roles: [Guard]

    task:
      index: 1
      source: Brood (larvas emiten continuamente), Queen
      emission_rate: 0.1/tick           # por larva o reina
      effect: modula transición a Nurse y Builder
      response_roles: [Nurse, Builder]

    attraction:
      index: 2
      source: Forager en celda con resource_amount > 0.3
      emission_rate: 0.15/tick
      effect: guía otras Foragers hacia recursos
      response_roles: [Forager]

  diffusion:
    kernel: gaussiano 3x3 separable (H + V)
    decay_rate: 0.05/tick               # concentration *= (1 - 0.05) cada tick
    boundary: absorción en bordes (no rebote, no toroide)
    buffer: double-buffering obligatorio

  max_concentration: 1.0               # se clampea, no hay desbordamiento
```

---

## Transición de Roles (Fixed-Threshold Response Model)

```yaml
RoleTransition:
  algorithm: Fixed-Threshold (Seeley 1995)
  formula: |
    stimulus_T = pheromone[tipo] × age_factor × health_factor
    P(adoptar T) = stimulus_T² / (stimulus_T² + threshold_T²)

  age_factor:
    Nurse:   max(0, 1 - age/400)        # decrece con edad
    Builder: gaussian(age, mean=250, sigma=100)
    Guard:   gaussian(age, mean=350, sigma=100)
    Forager: min(1, age/400)            # crece con edad

  health_factor:
    Susceptible: 1.0
    Infected:    0.5                    # infectados menos propensos a cambiar rol
    Recovered:   0.9

  threshold_heterogeneity: ±10%        # variación por agente desde seed determinista
  transition_cooldown: 30 ticks        # mínimo entre cambios de rol por agente

  constraints:
    - Queen nunca transiciona
    - Drone nunca transiciona
    - Máximo 1 reina activa por colmena
```

---

## Sistema Epidemiológico (SIR)

```yaml
Disease:
  states: [Susceptible, Infected, Recovered]

  transmission:
    formula: P(infección/tick) = base_rate × contact_density × susceptibility
    base_rate: 0.05                     # configurable por run
    contact_density: N° de Infected en celda + 8 celdas adyacentes
    susceptibility:
      Susceptible: 1.0
      Infected:    0.0                  # ya infectado
      Recovered:   0.0                  # inmune temporal
      age < 100:   0.8                  # cría reciente más resistente

  progression:
    infection_duration: 200 ticks       # configurable
    recovery_duration:  300 ticks       # duración de inmunidad antes de volver a S

  impact:
    energy_cost_multiplier: 1.20        # +20% coste metabólico
    foraging_efficiency: 0.70           # -30% ganancia en forrajeo
    role_transition_factor: 0.50        # -50% probabilidad de cambiar rol

  collapse_threshold: 0.60             # si >60% de población es Infected → colapso inminente
```

---

## Ciclo de Cría

```yaml
Brood:
  trigger: Queen deposita huevo si brood_production_rate > 0
  stages:
    Egg:   36 ticks
    Larva: 144 ticks   # requiere atención de Nurses
    Pupa:  120 ticks
  total_development: 300 ticks

  nurse_requirement:
    larva_needs_nurse: true
    if_no_nurse_adjacent: larva pierde 0.01 de "health virtual" / tick
    larva_dies_if: health_virtual <= 0 (equivalente a 100 ticks sin nodriza)

  eclosion:
    result: nueva Bee con Role=Nurse, Energy=0.8, Age=0, SirState=Susceptible
```

---

## System Dynamics (Variables Globales)

```yaml
SystemDynamics:
  update_frequency: cada 15 ticks

  variables:
    honey_reserve:
      type: f32 [0.0, ∞)
      effect: modula brood_production_rate (mínimo 0.2 para producción)
      drain: suma de energy_cost de todos los agentes / 100   # aproximación

    pollen_reserve:
      type: f32 [0.0, ∞)
      effect: afecta calidad de jalea real para larvas

    global_temp:
      type: f32 [°C]
      default: 20.0
      range: [-5.0, 45.0]
      effect: modula metabolic_cost_per_tick (ver Energy)

    season_phase:
      type: f32 [0.0, 1.0]   # 0.0=invierno, 0.5=verano, cíclico
      period: 86400 ticks    # ~1 día simulado = 1 año estacional
      effect: modula resource_amount en celdas, global_temp, brood_production_rate

    brood_production_rate:
      formula: base_rate * season_factor * reserve_factor
      base_rate: 0.1/tick
      season_factor: sin(season_phase * π)    # máximo en verano
      reserve_factor: clamp(honey_reserve / 0.5, 0.0, 1.0)

    pesticide_pressure:
      type: f32 [0.0, 1.0]   # 0=sin pesticidas
      effect:
        - Forager lifespan *= (1 - pesticide_pressure)
        - mortality_rate Forager += 0.001 * pesticide_pressure / tick

  feedbacks:
    - global_temp       → metabolic_cost_per_tick
    - honey_reserve     → brood_production_rate
    - colony_reserve < 0.30 → TrophallaxisSystem activado
    - season_phase      → resource_amount en grid
    - pesticide_pressure → mortalidad Forager
```

---

## Grid y Mundo

```yaml
World:
  size: 100 × 100 celdas
  cell_size: 10m × 10m
  total_area: 1 km²
  chunk_size: 16 × 16 celdas
  border_buffer: 5 celdas (absorción; agentes no pueden salir)

  hive_position: (50, 50)              # centro del grid por defecto
  food_sources:
    count: 3–8 (configurable)
    position: aleatoria a distancia > 20 celdas de la colmena
    initial_resource: 0.8
    regeneration_rate: 0.001/tick * season_factor
```

---

## Métricas de Emergencia

```yaml
Metrics:
  export_frequency: cada 60 ticks
  format: JSON + Parquet

  metrics:
    population_by_role:     distribución de agentes por Role
    colony_reserve:         honey_reserve normalizada
    mortality_rate:         muertes por causa (energy, disease, predation) / tick
    pheromone_entropy:      entropía espacial H = -Σ p(x,y) log p(x,y)
    foraging_efficiency:    Σ(forage_gain) / Σ(energy_cost_forager)
    brood_adult_ratio:      count(Brood) / count(Bee)
    sir_prevalence:         {S: %, I: %, R: %} sobre población total
    time_to_collapse:       tick en que population < collapse_threshold (si ocurre)

  collapse_threshold: 50 abejas adultas activas
```

---

## Parámetros Configurables por Run

```yaml
RunConfig:
  seed: u64                            # semilla global (determinismo)
  initial_population: 500             # abejas adultas al inicio
  initial_honey_reserve: 0.8
  disease:
    enabled: bool
    base_rate: f32
  pesticide_pressure: f32
  predator_count: u8
  season_start: f32                   # fase estacional inicial [0.0, 1.0]
  simulation_speed: u32               # ticks/s lógicos (default: 30)
```
