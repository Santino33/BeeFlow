# BeeFlow — Arquitectura Congelada

> Este documento es la **verdad técnica** del proyecto. Ningún agente puede modificar estas definiciones sin actualizar primero este archivo y justificar el cambio.

---

## Jerarquía del Mundo

```
World
 ├── Hive                        (entidad única, posición fija en el grid)
 ├── Bee[]                       (reina, nodrizas, recolectoras, guardianas, constructoras, zánganos)
 ├── Brood[]                     (huevos/larvas/pupas — agentes pasivos)
 ├── Predator[]                  (agentes activos externos)
 └── FoodSource[]                (celdas con recurso; no son entidades ECS, son datos del grid)
```

---

## Definiciones Fundamentales

### Entidad
Un `u32` (ID) que no contiene datos por sí mismo. Las entidades son gestionadas por `hecs`. No existen "tipos" de entidad — el conjunto de componentes es lo que determina el comportamiento.

### Componente
Una estructura de datos pura (`struct`) que almacena **estado**, no lógica. Ejemplo: `PositionComponent { x: u16, y: u16 }`. Los componentes no se llaman entre sí.

### Sistema
Una función que itera sobre entidades con un conjunto específico de componentes y **muta** su estado. Los sistemas no se comunican directamente entre sí: leen del estado del mundo y escriben de vuelta al mundo. El orden de ejecución está definido y es fijo (ver Orchestrator).

---

## Componentes por Tipo de Agente

### Bee (toda abeja activa)
| Componente | Tipo Rust | Descripción |
|---|---|---|
| `PositionComponent` | `(u16, u16)` | Celda actual en el grid |
| `RoleComponent` | `enum Role` | Rol vigente |
| `EnergyComponent` | `f32` | Energía actual `[0.0, 1.0]` |
| `HealthComponent` | `enum SirState` | Estado epidemiológico S/I/R |
| `AgeComponent` | `u32` | Edad en ticks |
| `PheromoneSensitivity` | `[f32; 3]` | Umbral de respuesta por tipo de feromona |
| `ForagerStateComponent` | `enum ForagerPhase` | Solo en Foragers. `Searching` o `Returning { carry: f32 }`. Gestionado por ForagingSystem y RoleTransitionSystem. |
| `RoleTransitionState` | `struct` | Solo en roles transicionables (Nurse/Builder/Guard/Forager). Ausente en Queen y Drone — la ausencia es la restricción estructural. Contiene `cooldown: u32` y `threshold_bias: f32`. |

### Brood (cría: huevo / larva / pupa)
| Componente | Tipo Rust | Descripción |
|---|---|---|
| `PositionComponent` | `(u16, u16)` | Celda donde reside |
| `AgeComponent` | `u32` | Ticks desde oviposición |
| `BroodStageComponent` | `enum BroodStage` | Egg / Larva / Pupa |

### Predator
| Componente | Tipo Rust | Descripción |
|---|---|---|
| `PositionComponent` | `(u16, u16)` | Celda actual |
| `EnergyComponent` | `f32` | Energía del depredador |
| `PredatorComponent` | `struct` | Tasa de ataque, radio de detección |

---

## Enumeraciones Canónicas

```rust
enum Role {
    Queen,
    Nurse,
    Builder,
    Guard,
    Forager,
    Drone,
}

enum SirState {
    Susceptible,
    Infected,
    Recovered,
}

enum BroodStage {
    Egg,
    Larva,
    Pupa,
}

enum ForagerPhase {
    Searching,
    Returning { carry: f32 },
}
```

---

## Estructura de la Celda del Grid

Las celdas **no son entidades ECS**. Son arrays contiguos en formato **SOA (Structure of Arrays)** gestionados por `SpatialGrid` para maximizar la localidad de caché:

```rust
// Representación lógica de una celda (x, y):
pheromone: [[f32; 2]; 3]    // [canal][buffer_A/B] — double-buffering
resource_amount: f32         // néctar/polen disponible [0.0, 1.0]
occupancy: u64               // contador de agentes en la celda
local_temp: f32              // temperatura local
is_obstacle: bool
```

En la implementación, cada campo es un `Vec<T>` separado indexado por `y * GRID_W + x`. La representación `struct Cell { ... }` es conceptual; el layout físico es SOA.

> **Nota de implementación (M1):** El diseño SOA fue elegido sobre AoS para el rendimiento de caché en operaciones de difusión y movimiento masivo. El backlog lo especificó explícitamente desde el inicio.

---

## Fuentes de Alimento

Las fuentes de alimento no son entidades ECS. Son celdas del grid con `resource_amount > 0`.

**Implementación actual (M7):**
- 3 fuentes en posiciones fijas: `(20, 50)`, `(80, 50)`, `(50, 20)`
- Cada fuente es un parche cuadrado de radio 8 (~17×17 celdas, ~10% del grid interior)
- `initial_resource: 1.0` por celda del parche
- Regeneración: `0.001/tick × season_factor`

> **Divergencia documentada respecto a `simulation_spec.md`:** La spec define puntos individuales configurables (3–8), posición aleatoria, `initial_resource: 0.8`. La implementación usa parches 17×17 con posiciones fijas porque con parches 3×3 los Foragers (vida ~40 ticks con costo 0.02/tick) no alcanzaban las fuentes en tiempo. Decisión de diseño ratificada en M7.

---

## Sistemas ECS y Orden de Ejecución

El orden dentro de cada tick es **fijo e inmutable**:

```
1. MovementSystem          — actualiza PositionComponent según rol y gradiente de feromona
2. AgeSystem               — incrementa AgeComponent en 1 por tick
3. EnergySystem            — aplica costo metabólico basal; modula por temperatura y pesticidas
4. ForagingSystem          — recolectoras consumen resource_amount; depositan en honey_reserve
5. TrophallaxisSystem      — redistribuye energía si honey_reserve < 0.30
6. DiseaseSystem           — transmisión SIR por contacto  [pendiente M12]
7. MortalitySystem         — elimina entidades con energy ≤ 0 o health terminal
8. RoleTransitionSystem    — evalúa Fixed-Threshold; puede cambiar RoleComponent
9. BroodSystem             — avanza BroodStage; eclosiona Pupa → nueva Bee adulta
10. PredatorSystem          — mueve depredadores; aplica ataques; emite feromona de alarma
```

Ningún sistema puede ejecutarse fuera de este orden dentro de un tick.

---

## Cómo Avanza el Tiempo

```
1 tick = 1 segundo simulado
Loop lógico: 30 ticks/segundo de reloj real
System Dynamics: se recalcula cada 15 ticks (tick % 15 == 0)
Métricas exportadas: cada 60 ticks (tick % 60 == 0)
```

El contador de tick es un `u64` global gestionado exclusivamente por el Orchestrator.

---

## Orden Completo del Orchestrator por Tick

```
1.  Leer inputs externos (config, eventos UI)
2.  Si tick % 15 == 0: actualizar System Dynamics
3.  Difundir feromonas en Grid (double-buffer swap)
4.  Regenerar recursos en celdas (según season_factor)
5.  Ejecutar sistemas ECS en orden canónico (paralelizable por chunks via rayon)
6.  Resolver interacciones Grid ↔ ECS (depositar feromonas, consumir recursos)
7.  Si tick % 60 == 0: exportar métricas
8.  Enviar estado al renderer vía mpsc (sin bloqueo)
9.  Incrementar tick counter
```

---

## Comunicación entre Capas

| De | Hacia | Canal | Frecuencia |
|---|---|---|---|
| System Dynamics | ECS Systems | Parámetros explícitos (`global_temp`, `pesticide_pressure`, `season_factor`) | Cada tick (valores actualizados cada 15 ticks) |
| Orchestrator | ForagingSystem | Parámetro `&mut honey_reserve` | Cada tick |
| ECS Systems | Grid | Escritura directa en buffer B de feromonas | Cada tick |
| Grid | ECS Systems | Lectura de buffer A (concentraciones, recursos) | Cada tick |
| System Dynamics | Grid | `season_factor` modula `resource_amount` vía `run_resource_regeneration` | Cada tick |
| Orchestrator | Renderer | `mpsc::channel` con estado serializado | Cada tick |

**Regla:** Los sistemas ECS no se llaman entre sí. Toda comunicación pasa por el estado del mundo (Grid, componentes, o parámetros explícitos del Orchestrator).

> **Nota sobre `honey_reserve`:** Es un campo del `Orchestrator`, no de `SystemDynamicsState`. Esto es intencional: `honey_reserve` es modificado por `ForagingSystem` (escritura directa) y leído por `TrophallaxisSystem` y `SystemDynamicsState.update()`. Centralizarlo en el Orchestrator evita dependencias circulares entre SystemDynamics y los sistemas ECS.

---

## Thread Model

```
Thread 0 (principal):  Orchestrator loop — tick lógico
Thread 1 (render):     Consume mpsc channel — 60 Hz, interpolación lineal entre ticks
Threads rayon:         Pool para sistemas ECS y difusión de feromonas (thread-local RNG)
```

No hay locks en el hot path. El único punto de sincronización es el swap del double-buffer (operación atómica).

---

## Contratos de Interfaz entre Sistemas

Cada sistema solo puede leer/escribir los componentes que le corresponden:

| Sistema | Lee | Escribe |
|---|---|---|
| MovementSystem | `Position, Role, PheromoneSensitivity, ForagerStateComponent, Grid(pheromone)` | `Position, Grid(occupancy)` |
| AgeSystem | `Age` | `Age` |
| EnergySystem | `Energy, Role` + `global_temp, pesticide_pressure` (params) | `Energy` |
| ForagingSystem | `Position, Role, Energy, ForagerStateComponent, Grid(resource)` | `Energy, ForagerStateComponent, Grid(resource, pheromone), honey_reserve` |
| TrophallaxisSystem | `Position, Energy` + `honey_reserve` (param) | `Energy` |
| DiseaseSystem | `Position, Health` | `Health` |
| MortalitySystem | `Energy` | _(elimina entidad)_, `Grid(occupancy)` |
| RoleTransitionSystem | `Position, Role, Age, Health, PheromoneSensitivity, RoleTransitionState, Grid(pheromone)` | `Role, PheromoneSensitivity, ForagerStateComponent, RoleTransitionState` |
| BroodSystem | `BroodStage, Age` | `BroodStage`, _(spawn Bee)_ |
| PredatorSystem | `Position(pred), Position(bee), Energy` | `Energy(bee), Grid(pheromone)` |

Un sistema que lee o escribe fuera de su contrato es un **bug de arquitectura**.
