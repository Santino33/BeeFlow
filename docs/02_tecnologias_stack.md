# BeeFlow — Especificaciones Técnicas y Stack Tecnológico

## Contexto del Hardware Objetivo

BeeFlow v1 está diseñado para ejecutarse en hardware de consumo, sin depender de infraestructura especializada:

| Recurso | Especificación Objetivo |
|---|---|
| CPU | 6–16 núcleos (x86_64 o ARM) |
| RAM | 16–32 GB |
| GPU | Integrada o gama media (no requerida en v1) |
| OS | Linux / Windows / macOS (nativo) |
| Presupuesto de tick | < 80 ms en CPU 8-core |
| Velocidad de simulación | >30× el tiempo real |

---

## Stack Tecnológico Seleccionado

### Lenguaje Principal: Rust

Rust es la base del proyecto por sus garantías de seguridad de memoria sin garbage collector, sus zero-cost abstractions y su ecosistema maduro para computación de alto rendimiento.

**Configuración de compilación (perfil release):**
```toml
[profile.release]
lto = true
codegen-units = 1
panic = "abort"
opt-level = 3
```

---

### ECS (Entity Component System): `hecs`

Se utiliza **hecs** como librería ECS por ser ligera, cache-friendly y sin acoplamiento a un motor mayor.

- **Alternativa evaluada y descartada:** `bevy_ecs` (acoplado al motor Bevy, overhead innecesario), `legion` (mantenimiento lento).
- **Consideración:** Si la lógica de agentes resulta predecible y estática, se puede migrar a **SOA puro** (`Vec<Componente>` + `rayon`) para máxima localidad de caché, eliminando el overhead de ECS formal.

---

### Grid Espacial: Chunked Grid + Spatial Hashing

El mundo 2D se representa como un **grid chunked** (bloques de 16×16 celdas), con hashing espacial para consultas de vecindad eficientes.

- **Descartado:** Matriz densa con `Vec<EntityID>` por celda (genera fragmentación, cache misses y contención en paralelo).
- **Representación de ocupación:** Bitsets de ocupación o listas externas indexadas (no `Vec` dinámicos por celda).
- **Librería de referencia:** crate `grid` o implementación propia.

**Parámetros de mundo (v1):**
```
Tamaño lógico:  1 km × 1 km
Resolución:     celdas de 10 m × 10 m
Dimensiones:    grid 100 × 100 celdas
Chunks:         bloques 16 × 16
Zona borde:     5 celdas de amortiguamiento (límites duros)
```

---

### Paralelismo: `rayon`

**rayon** gestiona el paralelismo de datos mediante iteradores paralelos sobre chunks del grid y pools de agentes.

Estrategias anti-contención:
- **Double-buffering** para grid y agentes: un buffer se lee mientras el otro se escribe.
- **Thread-local RNG** (`rand_xoshiro256**`) para evitar contención en generación de números aleatorios.
- **Sin locks globales:** se usan colas lock-free (`crossbeam`) únicamente para eventos asíncronos puntuales.

---

### Difusión de Feromonas

**Modelo matemático elegido:** Concentración continua por celda con kernel gaussiano 3×3 + decaimiento exponencial.

```
decay_rate = 0.05 por tick
kernel:     gaussiano 3×3 normalizado
método:     kernels separables (paso horizontal + paso vertical)
buffer:     double-buffering obligatorio (sin lectura/escritura simultánea)
librería:   ndarray + rayon
```

**Descartado:** difusión ingenua sin kernel (genera oscilaciones numéricas) y transporte advectivo (complejidad innecesaria en v1).

---

### System Dynamics: `faer` / `odes`

Las variables macroeconómicas de la colonia (reservas globales, estacionalidad, tasas de cría) se resuelven con integración de ecuaciones diferenciales.

- **Librería:** `faer` para álgebra lineal; `odes` para integración de ODEs si el sistema lo requiere.
- **Frecuencia:** Las variables globales se recalculan cada **15 ticks** para desacoplar escalas temporales y reducir carga computacional.

**Retroalimentaciones implementadas:**
```
global_temp        → modula metabolic_cost_per_tick
honey_reserve      → modula brood_production_rate
colony_reserve < 30% → activa trofalaxia entre agentes
```

---

### Visualización: `egui` + `wgpu` / `macroquad`

Para v1 se prioriza la visualización científica ligera sobre la inmersividad visual:

| Opción | Descripción | Cuándo usar |
|---|---|---|
| **egui + wgpu** | Render nativo GPU, dashboards, gráficas, inspección de agentes | Opción principal (recomendada) |
| **macroquad** | Render 2D simple, prototipado rápido | Alternativa para prototipos |
| **Godot (godot-rust)** | Motor visual completo vía GDExtension | Diferido a v2 si se prioriza UX |

**Arquitectura de render desacoplada:**
- La simulación corre en un **thread dedicado**.
- El visualizador consume el estado vía `mpsc channel` con interpolación lineal entre ticks.
- Se busca **zero-copy** del estado cuando sea posible.

---

### Profiling y Observabilidad

| Herramienta | Propósito |
|---|---|
| `tracing` + `tracing-flame` | Trazado de ejecución y flamegraphs |
| `criterion` | Benchmarks estadísticos de componentes |
| `heaptrack` | Análisis de uso y fragmentación de memoria |

---

### Reproducibilidad y RNG

- **Generador:** `rand_xoshiro256**` (rápido, estadísticamente robusto, paralelizable)
- **Semilla global:** configurable por run desde archivo de configuración
- **Semillas por agente:** generadas determinísticamente desde `seed_global + agent_id`
- Esto garantiza que dos runs con la misma semilla produzcan resultados idénticos

---

### Exportación de Datos

- **Formato:** JSON estructurado y/o Parquet (columnar, eficiente para análisis)
- **Frecuencia:** cada 60 ticks (~2 segundos de simulación)
- **Métricas exportadas:**
  - Población por rol (recolectora, nodriza, guardiana, etc.)
  - Reservas energéticas de la colonia
  - Tasa de mortalidad por causa
  - Entropía espacial de feromonas
  - Tiempo hasta colapso o recuperación
  - Estado epidemiológico (S, I, R por agente/zona)

---

## Paso de Tiempo y Escalas Temporales

```
1 tick = 1 segundo simulado
Motor lógico: 30 ticks/s
Render: 60 Hz (desacoplado del tick lógico)
Variables globales (System Dynamics): cada 15 ticks
```

El paso de tiempo fijo garantiza estabilidad numérica y reproducibilidad en sistemas estocásticos.

---

## Resumen del Stack

```
Lenguaje:        Rust (release hardened)
ECS:             hecs  (o SOA + rayon puro si aplica)
Grid:            Chunked 16×16 + spatial hashing
Paralelismo:     rayon + double-buffering + thread-local RNG
Feromonas:       ndarray + rayon (kernels separables, double-buffer)
System Dynamics: faer / odes  (cada 15 ticks)
Visualización:   egui + wgpu  (thread de render separado)
Profiling:       tracing, criterion, heaptrack
RNG:             rand_xoshiro256**  (seed determinista)
Exportación:     JSON / Parquet  (cada 60 ticks)
GPU compute:     No en v1  (reservado para >50k agentes o grids >500×500)
```

---

## Decisiones Diferidas (No en v1)

- **GPU compute** (CUDA/WGPU): solo necesario para escala masiva
- **Despliegue web**: pospuesto por limitaciones de threading y debugging científico; se contempla cliente WebAssembly/WebGPU como visualizador remoto en versiones futuras
- **Godot**: integración posible en v2 si se prioriza experiencia visual
