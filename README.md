# BeeFlow — Índice de Documentación

Simulación computacional híbrida de una colonia de abejas *Apis mellifera*.

---

## Documentos del Proyecto

| Archivo | Contenido |
|---|---|
| `01_vision_general.md` | ¿Qué es BeeFlow? Motivación, objetivos, alcance de v1 y visión a largo plazo |
| `02_tecnologias_stack.md` | Stack tecnológico completo, justificación de decisiones, parámetros de hardware objetivo |
| `03_arquitectura_capas.md` | Las cuatro capas de simulación, su responsabilidad, frecuencia y protocolos de acoplamiento |
| `04_modelos_biologicos.md` | Modelos matemáticos y biológicos: energía, roles, feromonas, SIR, cría, métricas de emergencia |
| `05_hoja_de_ruta.md` | Fases de desarrollo, criterios de éxito por fase y features futuras |

---

## Resumen Técnico Rápido

```
Lenguaje:       Rust (release hardened)
ECS:            hecs
Grid:           Chunked 16×16 + spatial hashing  (100×100 celdas, 1 km²)
Paralelismo:    rayon + double-buffering
Feromonas:      ndarray + kernel gaussiano 3×3 + decaimiento 0.05/tick
System Dynamics:faer / odes  (cada 15 ticks)
Visualización:  egui + wgpu  (thread separado)
RNG:            rand_xoshiro256**  (seed determinista por run)
Exportación:    JSON / Parquet  (cada 60 ticks)
Tick:           1 tick = 1 segundo simulado  |  30 ticks/s lógicos
Agentes v1:     5,000–8,000 simultáneos
Presupuesto:    < 80 ms/tick  |  >30× tiempo real
```

---

## Guía de Lectura

- **Nuevo en el proyecto →** empieza por `01_vision_general.md`
- **Implementando el motor →** `03_arquitectura_capas.md` + `04_modelos_biologicos.md`
- **Eligiendo librerías →** `02_tecnologias_stack.md`
- **Planificando sprints →** `05_hoja_de_ruta.md`
