# BeeFlow — Matriz de Complejidad y Prioridad

> Guía de implementación: de arriba hacia abajo dentro de cada iteración. No implementar una fila hasta que las de mayor prioridad en su iteración estén completas y dentro de presupuesto.

---

## Iteración 1 — Mundo y Agentes Mínimos

| Módulo | Complejidad | Prioridad | Estado | Módulo backlog |
|---|---|---|---|---|
| Fundaciones Rust / Orchestrator | Baja | Crítica | Pendiente | M0 |
| Grid 100×100 chunked | Baja | Crítica | Pendiente | M1 |
| Difusión de feromonas | Media | Crítica | Pendiente | M2 |
| Agentes ECS básicos (spawn, move, age) | Baja | Crítica | Pendiente | M3 |

---

## Iteración 2 — Energía y Muerte

| Módulo | Complejidad | Prioridad | Estado | Módulo backlog |
|---|---|---|---|---|
| Sistema energético (metabolismo, forrajeo básico) | Baja | Alta | Pendiente | M4 |
| Mortalidad y reuso de IDs | Baja | Alta | Pendiente | M5 |

---

## Iteración 3 — Roles y Comportamiento Social

| Módulo | Complejidad | Prioridad | Estado | Módulo backlog |
|---|---|---|---|---|
| Roles diferenciados (costos, movimiento por rol) | Media | Alta | Pendiente | M6 |
| Sistema de forrajeo completo | Media | Alta | Pendiente | M7 |
| Transición de roles (Fixed-Threshold) | Alta | Alta | Pendiente | M8 |
| Trofalaxia | Baja | Media | Pendiente | M9 |

---

## Iteración 4 — System Dynamics

| Módulo | Complejidad | Prioridad | Estado | Módulo backlog |
|---|---|---|---|---|
| Variables globales y estacionalidad | Media | Alta | Pendiente | M10 |
| Ciclo de cría (huevo → larva → pupa → adulto) | Alta | Alta | Pendiente | M11 |

---

## Iteración 5 — Perturbaciones

| Módulo | Complejidad | Prioridad | Estado | Módulo backlog |
|---|---|---|---|---|
| Sistema de enfermedades SIR | Alta | Media | Pendiente | M12 |
| Depredadores | Media | Media | Pendiente | M13 |
| Presión de pesticidas | Baja | Media | Pendiente | M10 (integrado) |

---

## Iteración 6 — Visualización y Exportación

| Módulo | Complejidad | Prioridad | Estado | Módulo backlog |
|---|---|---|---|---|
| Exportación completa JSON/Parquet | Baja | Alta | Pendiente | M14 |
| Visualizador egui + wgpu | Alta | Media | Pendiente | M15 |
| Reproducción de runs (replay) | Media | Media | Pendiente | M15 |

---

## Post-v1 (Diferido)

| Feature | Complejidad | Prioridad | Versión |
|---|---|---|---|
| GPU compute (difusión + ECS en shaders) | Muy alta | Baja | v2 |
| Grids > 500×500 | Alta | Baja | v2 |
| Dashboard web (WebAssembly/WebGPU) | Muy alta | Baja | v2 |
| Integración Godot (godot-rust) | Muy alta | Baja | v2 (opcional) |
| Termorregulación activa de la colmena | Alta | Baja | v2 |
| Variabilidad genética entre colonias | Muy alta | Baja | v3 |
| Multi-colonia (competencia por recursos) | Muy alta | Baja | v3 |
| Calibración automática de parámetros | Alta | Baja | v3 |
| Enjambrazón (swarming) | Muy alta | Baja | v3 |
| Modelado específico de Varroa/Nosema | Alta | Baja | v3 |
| Inmunidad materna transmitida a cría | Media | Baja | v3 |

---

## Presupuesto de Rendimiento por Módulo

| Componente | Presupuesto | Medición |
|---|---|---|
| Grid 100×100 vacío (difusión) | < 5 ms/tick | criterion benchmark |
| 5,000 agentes (todos los sistemas ECS) | < 40 ms/tick | criterion benchmark |
| System Dynamics (cada 15 ticks) | < 5 ms/ejecución | criterion benchmark |
| Exportación de métricas (cada 60 ticks) | < 10 ms/ejecución | criterion benchmark |
| Renderer (thread separado) | < 16 ms/frame | medición interna egui |
| **Total thread de simulación** | **< 80 ms/tick** | tracing-flame |

Un módulo que supera su presupuesto **no pasa a producción** hasta que se optimiza.

---

## Criterios de Cambio de Estado

Un módulo pasa de **Pendiente → En progreso → Completo** cuando:

1. **En progreso:** el agente responsable tiene el RFC aprobado (si aplica) y comenzó implementación.
2. **Completo:** 
   - Todos los ítems del checklist del backlog marcados.
   - Criterio de éxito verificado por QA.
   - Benchmark dentro del presupuesto.
   - Sin warnings de Clippy en modo release.
   - Dos runs con misma seed producen resultados idénticos.

Actualizar esta tabla cuando un módulo cambia de estado.
