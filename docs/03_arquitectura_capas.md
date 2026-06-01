# BeeFlow — Arquitectura de Capas

## Visión General

BeeFlow integra cuatro capas de simulación que operan de forma sincronizada dentro de un ciclo de tick controlado por el **Simulation Orchestrator**. Cada capa tiene una responsabilidad diferenciada, una frecuencia de actualización definida y protocolos explícitos de comunicación con las demás.

```
╔══════════════════════════════════════════════════════╗
║              SIMULATION ORCHESTRATOR                 ║
║  Controla el ciclo de tick, sincroniza capas,        ║
║  gestiona clima global y exporta métricas            ║
╠═════════════════════╦════════════════════════════════╣
║  ECS AGENT LAYER    ║  SYSTEM DYNAMICS LAYER         ║
║  Agentes individuales║  Variables macroeconómicas    ║
║  (abejas, deps.)    ║  (reservas, estaciones, clima) ║
╠═════════════════════╩════════════════════════════════╣
║                SPATIAL GRID LAYER                    ║
║   Entorno 2D: feromonas, recursos, clima local,      ║
║   ocupación espacial y condiciones de contorno       ║
╚══════════════════════════════════════════════════════╝
```

---

## Capa 1 — ECS Agent Layer

**Responsabilidad:** Representar y actualizar el estado de cada agente individual (abeja, depredador, cría).

**Tecnología:** `hecs` (Entity Component System ligero y cache-friendly)

### Componentes Principales por Agente

| Componente | Descripción |
|---|---|
| `PositionComponent` | Coordenadas (x, y) en el grid |
| `RoleComponent` | Rol actual: recolectora, nodriza, guardiana, constructora, zángano, reina |
| `EnergyComponent` | Energía actual, capacidad máxima (1.0 units) |
| `HealthComponent` | Estado de salud: S (susceptible), I (infectado), R (recuperado) |
| `AgeComponent` | Edad en ticks; modula transición de roles |
| `PheromoneSensitivity` | Umbral de respuesta a señales químicas por tipo |

### Sistemas ECS (se ejecutan cada tick)

1. **MovementSystem** — Actualiza posición según rol y señales del grid
2. **EnergySystem** — Aplica costo metabólico basal y ganancias por forrajeo
3. **RoleTransitionSystem** — Evalúa umbrales para cambio de rol
4. **ForagingSystem** — Recolectoras interactúan con celdas de recursos
5. **TrophallaxisSystem** — Transferencia de energía entre agentes próximos si `colony_reserve < 30%`
6. **DiseaseSystem** — Transmisión por contacto según el modelo SIR
7. **MortalitySystem** — Elimina agentes cuya energía o salud llega a cero

### Frecuencia de actualización
Todos los sistemas ECS se ejecutan **cada tick** (1 segundo simulado).

---

## Capa 2 — Spatial Grid Layer

**Responsabilidad:** Modelar el entorno físico donde los agentes existen e interactúan.

**Tecnología:** Grid chunked (bloques 16×16) + spatial hashing

### Contenido por Celda

| Campo | Tipo | Descripción |
|---|---|---|
| `pheromone[tipo]` | `f32` | Concentración de feromona por tipo (alarma, tarea, atracción) |
| `resource_amount` | `f32` | Cantidad de néctar/polen disponible |
| `occupancy` | `bitset` | Presencia de agentes (evita Vec dinámico) |
| `local_temp` | `f32` | Temperatura local de la celda |
| `is_obstacle` | `bool` | Indica si la celda es impasable |

### Difusión de Feromonas (cada tick)

```
Algoritmo:  Kernel gaussiano 3×3 (kernels separables: H + V)
Decaimiento: exponencial — concentration *= (1 - 0.05) por tick
Buffer:     double-buffering (buffer_A se lee, buffer_B se escribe; swap al final)
Librería:   ndarray + rayon (paralelizado por chunks)
```

### Condiciones de Contorno

- **Bordes duros** con **zona de amortiguamiento de 5 celdas** alrededor del grid
- Las feromonas que alcanzan el borde se absorben (no hay rebote ni toroide)
- Los agentes no pueden atravesar el borde exterior

---

## Capa 3 — System Dynamics Layer

**Responsabilidad:** Gestionar las variables macroeconómicas de la colonia y las condiciones ambientales globales.

**Tecnología:** `faer` / `odes` para integración de ODEs

### Variables Globales

| Variable | Descripción | Efecto |
|---|---|---|
| `honey_reserve` | Reserva global de miel (unidades arbitrarias) | Modula `brood_production_rate` |
| `pollen_reserve` | Reserva de polen | Afecta producción de jalea real |
| `global_temp` | Temperatura ambiental global | Modula `metabolic_cost_per_tick` |
| `season_phase` | Fase estacional (0.0–1.0, cíclica) | Afecta floración, temperatura, fotoperíodo |
| `brood_production_rate` | Tasa de eclosión de nueva cría | Depende de reservas y temperatura |
| `pesticide_pressure` | Presión por pesticidas en el entorno | Incrementa mortalidad y reduce forrajeo |

### Frecuencia de Actualización

Las variables de System Dynamics se recalculan **cada 15 ticks** para desacoplar escalas temporales y reducir carga computacional. Entre actualizaciones, los valores se interpolan linealmente cuando son consumidos por ECS.

### Retroalimentaciones Definidas

```
global_temp          → metabolic_cost_per_tick  (relación positiva)
honey_reserve        → brood_production_rate     (relación positiva, umbral mínimo)
colony_reserve < 30% → activa trofalaxia generalizada
pesticide_pressure   → reduce lifespan de recolectoras, aumenta mortalidad
season_phase         → modula resource_amount en celdas del grid
```

---

## Capa 4 — Simulation Orchestrator

**Responsabilidad:** Controlar el ciclo maestro de tick, garantizar el orden de actualización de capas y coordinar la exportación de datos.

### Orden de Ejecución por Tick

```
1.  Recibir inputs externos (configuración, eventos de usuario)
2.  Actualizar System Dynamics  (si tick % 15 == 0)
3.  Difundir feromonas en el Grid  (double-buffer swap)
4.  Regenerar recursos en celdas   (según season_phase)
5.  Ejecutar sistemas ECS          (en paralelo por chunks via rayon)
    └─ Movement → Energy → Foraging → Trophallaxis → Disease → Mortality → Role
6.  Resolver interacciones Grid ↔ ECS  (depositar feromonas, consumir recursos)
7.  Exportar métricas              (si tick % 60 == 0)
8.  Enviar estado al renderer      (vía mpsc channel, sin bloqueo)
9.  Incrementar contador de tick
```

### Protocolo de Acoplamiento entre Capas

| Dirección | Frecuencia | Mecanismo |
|---|---|---|
| System Dynamics → ECS | Cada 15 ticks | Variables globales como constantes de sistemas |
| ECS → Grid | Cada tick | Escritura en buffer de feromona y ocupación |
| Grid → ECS | Cada tick | Lectura de concentraciones y recursos por posición |
| System Dynamics → Grid | Cada 15 ticks | Actualización de `local_temp` y `resource_amount` |
| Orchestrator → Renderer | Cada tick | Estado serializado vía `mpsc` (sin bloqueo, zero-copy cuando aplica) |

---

## Thread Model

```
Thread Principal (Simulación):
  └─ Orchestrator loop: tick lógico a 30 ticks/s

Thread de Render:
  └─ Consume estado vía mpsc channel
  └─ Interpola entre ticks para render fluido a 60 Hz
  └─ No bloquea ni ralentiza el loop de simulación

Thread-local RNG:
  └─ Cada thread de rayon tiene su propio rand_xoshiro256**
  └─ Sin contención en generación de números aleatorios
```

---

## Notas de Diseño

- **Caché primero:** Toda estructura de datos prioriza localidad de caché (SOA sobre AOS, chunking sobre matriz densa plana, bitsets sobre Vec).
- **Sin locks en el hot path:** Los únicos puntos de sincronización son el double-buffer swap (atómico) y el channel hacia el renderer.
- **Determinismo garantizado:** Con la misma semilla, dos ejecuciones producen resultados bit a bit idénticos.
