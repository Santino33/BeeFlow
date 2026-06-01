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
```

---

## Estructura de una Celda del Grid

Cada celda `(x, y)` del grid 100×100 almacena:

```rust
struct Cell {
    pheromone: [f32; 3],   // [alarm, task, attraction]
    resource_amount: f32,  // néctar/polen disponible [0.0, 1.0]
    occupancy: u64,        // bitset de ocupación (hasta 64 agentes/celda)
    local_temp: f32,       // temperatura local
    is_obstacle: bool,
}
```

Las celdas **no son entidades ECS**. Son un array contiguo gestionado por el `SpatialGrid`.

---

## Sistemas ECS y Orden de Ejecución

El orden dentro de cada tick es **fijo e inmutable**:

```
1. MovementSystem          — actualiza PositionComponent según rol y gradiente de feromona
2. EnergySystem            — aplica costo metabólico basal; aplica ganancia de recolectoras
3. ForagingSystem          — recolectoras consumen resource_amount de su celda
4. TrophallaxisSystem      — redistribuye energía si colony_reserve < 0.30
5. DiseaseSystem           — transmisión SIR por contacto
6. MortalitySystem         — elimina entidades con energy ≤ 0 o health terminal
7. RoleTransitionSystem    — evalúa Fixed-Threshold; puede cambiar RoleComponent
8. BroodSystem             — avanza BroodStage; eclosiona Pupa → nueva Bee adulta
9. PredatorSystem          — mueve depredadores; aplica ataques; emite feromona de alarma
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
4.  Regenerar recursos en celdas (según season_phase)
5.  Ejecutar sistemas ECS en orden canónico (paralelizado por chunks via rayon)
6.  Resolver interacciones Grid ↔ ECS (depositar feromonas, consumir recursos)
7.  Si tick % 60 == 0: exportar métricas
8.  Enviar estado al renderer vía mpsc (sin bloqueo)
9.  Incrementar tick counter
```

---

## Comunicación entre Capas

| De | Hacia | Canal | Frecuencia |
|---|---|---|---|
| System Dynamics | ECS Systems | Variables globales (lectura directa) | Cada 15 ticks |
| ECS Systems | Grid | Escritura directa en buffer B de feromonas | Cada tick |
| Grid | ECS Systems | Lectura de buffer A (concentraciones, recursos) | Cada tick |
| System Dynamics | Grid | Actualiza `local_temp` y `resource_amount` | Cada 15 ticks |
| Orchestrator | Renderer | `mpsc::channel` con estado serializado | Cada tick |

**Regla:** Los sistemas ECS no se llaman entre sí. Toda comunicación pasa por el estado del mundo (Grid o componentes).

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
| MovementSystem | Position, Role, Grid(pheromone) | Position |
| EnergySystem | Energy, Role, GlobalState | Energy |
| ForagingSystem | Position, Role, Energy | Energy, Grid(resource) |
| TrophallaxisSystem | Position, Energy, GlobalState | Energy |
| DiseaseSystem | Position, Health | Health |
| MortalitySystem | Energy, Health | (elimina entidad) |
| RoleTransitionSystem | Age, Role, PheromoneSensitivity, Grid(pheromone) | Role |
| BroodSystem | BroodStage, Age | BroodStage, (spawns Bee) |
| PredatorSystem | Position(pred), Position(bee), Energy | Energy(bee), Grid(pheromone) |

Un sistema que lee o escribe fuera de su contrato es un **bug de arquitectura**.
