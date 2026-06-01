# BeeFlow — Hoja de Ruta y Fases de Desarrollo

## Filosofía de Desarrollo

BeeFlow prioriza **núcleo correcto antes que features completas**. Cada fase debe entregar un sistema que sea científicamente coherente, medible y reproducible antes de añadir complejidad. No se añade un sistema nuevo hasta que el anterior pasa sus benchmarks de rendimiento y sus métricas de emergencia son razonables.

---

## Fase 0 — Fundaciones (Infraestructura)

**Objetivo:** Establecer el entorno de desarrollo, el ciclo de tick base y las herramientas de observabilidad antes de escribir lógica de simulación.

### Tareas

- [ ] Inicializar proyecto Rust con configuración de release hardened
- [ ] Integrar `hecs`, `rayon`, `ndarray`, `tracing`
- [ ] Implementar Simulation Orchestrator: loop de tick fijo (30 ticks/s lógicos), desacoplado del render
- [ ] Implementar exportación de métricas (JSON, cada 60 ticks)
- [ ] Configurar `criterion` para benchmarks de componentes críticos
- [ ] Configurar `tracing-flame` para flamegraphs de profiling
- [ ] Implementar sistema de semillas configurable (`rand_xoshiro256**`)

### Criterio de Éxito
- El loop de tick corre vacío a < 1 ms/tick en CPU objetivo
- El sistema de logs y exportación no introduce jitter medible

---

## Fase 1 — Grid y Feromonas

**Objetivo:** Tener un grid funcional con difusión de feromonas correcta y medible.

### Tareas

- [ ] Implementar grid chunked 100×100 (bloques 16×16)
- [ ] Implementar spatial hashing para consultas de vecindad
- [ ] Implementar representación de celdas: feromonas, recursos, ocupación (bitsets)
- [ ] Implementar difusión por kernel gaussiano 3×3 separable con double-buffering
- [ ] Implementar decaimiento exponencial por tick
- [ ] Implementar condiciones de contorno (bordes duros + zona de amortiguamiento de 5 celdas)
- [ ] Benchmark: difusión en grid 100×100 completo < 5 ms/tick en 8 cores

### Criterio de Éxito
- La entropía espacial de feromonas se comporta como se espera (concentración, difusión, decaimiento)
- Sin condiciones de carrera con Helgrind o ThreadSanitizer

---

## Fase 2 — Agentes Básicos (ECS)

**Objetivo:** Agentes que se mueven, consumen energía y mueren.

### Tareas

- [ ] Definir componentes ECS: `Position`, `Role`, `Energy`, `Health`, `Age`
- [ ] Implementar `MovementSystem` (movimiento aleatorio + bias por gradiente de feromona)
- [ ] Implementar `EnergySystem` (costo basal, muerte por energía agotada)
- [ ] Implementar pool de agentes con reuso de IDs (evitar fragmentación)
- [ ] Benchmark: 5,000 agentes con todos los sistemas activos < 40 ms/tick

### Criterio de Éxito
- Población de 5,000 agentes con movimiento y energía: < 40 ms/tick
- El perfil de mortalidad sin recarga de energía es coherente con una vida útil biológicamente razonable

---

## Fase 3 — Roles y Comportamiento Social

**Objetivo:** Abejas con roles diferenciados que exhiben comportamiento emergente básico.

### Tareas

- [ ] Implementar todos los roles (reina, nodriza, recolectora, guardiana, constructora, zángano)
- [ ] Implementar `RoleTransitionSystem` (Fixed-Threshold con modulación por edad y feromona)
- [ ] Implementar `ForagingSystem` (recolectoras interactúan con celdas de recursos)
- [ ] Implementar `TrophallaxisSystem` (transferencia de energía bajo umbral del 30%)
- [ ] Implementar deposición de feromonas por rol

### Criterio de Éxito
- La distribución de roles evoluciona sin intervención externa hacia proporciones biológicamente razonables
- El sistema de roles no colapsa a un estado uniforme

---

## Fase 4 — System Dynamics y Acoplamiento

**Objetivo:** Variables macroeconómicas conectadas con la dinámica de agentes.

### Tareas

- [ ] Implementar variables globales: `honey_reserve`, `pollen_reserve`, `global_temp`, `season_phase`
- [ ] Implementar actualización de System Dynamics cada 15 ticks
- [ ] Implementar retroalimentaciones: temp→metabolismo, reservas→cría, estación→recursos del grid
- [ ] Implementar ciclo de cría: huevos, desarrollo, eclosión como adultos

### Criterio de Éxito
- La colonia es capaz de crecer, estabilizarse y declinar según las estaciones
- Las reservas muestran ciclos coherentes con la estacionalidad configurada

---

## Fase 5 — Perturbaciones: Enfermedades y Depredadores

**Objetivo:** Amenazas que afectan realmente la dinámica de la colonia.

### Tareas

- [ ] Implementar modelo SIR: transmisión por contacto en grid, estados S/I/R por agente
- [ ] Implementar impacto de infección: aumento de costo energético, reducción de eficiencia
- [ ] Implementar agentes depredadores: movimiento, ataque, respuesta de feromonas de alarma
- [ ] Implementar presión de pesticidas: variable global que afecta mortalidad de recolectoras

### Criterio de Éxito
- Una enfermedad no tratada puede llevar la colonia al colapso en condiciones de escasez
- Una colonia sana puede resistir brotes leves sin colapsar

---

## Fase 6 — Visualización y Exportación

**Objetivo:** Herramientas para observar, analizar y publicar resultados.

### Tareas

- [ ] Integrar visualizador `egui + wgpu` en thread separado
- [ ] Visualizar grid con colores por concentración de feromona y recursos
- [ ] Panel de métricas en tiempo real (población, reservas, mortalidad)
- [ ] Implementar exportación Parquet de todas las métricas definidas
- [ ] Implementar reproducción de runs guardados (replay desde seed + config)

### Criterio de Éxito
- El visualizador no introduce más de 2 ms de overhead en el thread de simulación
- Los datos exportados permiten reproducir cualquier run con idénticos resultados

---

## Versiones Futuras (Post-v1)

| Feature | Versión | Descripción |
|---|---|---|
| GPU Compute | v2 | Difusión y lógica ECS en compute shaders para >50k agentes |
| Grids grandes | v2 | Soporte para grids >500×500 con chunking jerárquico |
| Dashboard web | v2 | Cliente WebAssembly/WebGPU como visualizador remoto desacoplado |
| Godot Integration | v2 (opcional) | Visualización inmersiva vía godot-rust (GDExtension) |
| Variabilidad genética | v3 | Diferencias heredables entre colonias para estudios evolutivos |
| Multi-colonia | v3 | Varias colonias compitiendo por recursos en el mismo grid |
| Calibración automática | v3 | Ajuste de parámetros por comparación con datos empíricos |

---

## Presupuesto de Rendimiento por Fase

| Componente | Presupuesto (8-core, 16 GB RAM) |
|---|---|
| Grid 100×100 vacío (difusión) | < 5 ms/tick |
| 5,000 agentes (todos los sistemas) | < 40 ms/tick |
| System Dynamics (cada 15 ticks) | < 5 ms por ejecución |
| Exportación de métricas (cada 60 ticks) | < 10 ms por ejecución |
| Render (thread separado) | < 16 ms/frame (60 fps) |
| **Total objetivo** | **< 80 ms/tick en el thread de simulación** |
