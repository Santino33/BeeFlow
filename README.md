# BeeFlow

Simulacion computacional hibrida de una colonia de abejas (*Apis mellifera*) con arquitectura ECS + grid de campo + dinamica global.

El proyecto esta orientado a:
- simulacion reproducible por semilla,
- comportamiento emergente multi-agente,
- experimentacion con metricas exportables,
- visualizacion en tiempo real opcional.

## Inicio rapido

Requisitos:
- Rust estable (toolchain `stable`)
- Cargo

Ejecutar en modo headless (sin GUI):

```bash
cargo run --release -- --no-gui --max-ticks 3600 --seed 42 --speed 30
```

Ejecutar con visualizador (`egui`):

```bash
cargo run --release --features visualizer -- --max-ticks 3600 --seed 42 --speed 30
```

Probar el proyecto:

```bash
cargo test
```

Benchmarks principales:

```bash
cargo bench
```

## Argumentos de ejecucion

Actualmente el binario soporta:
- `--seed <u64>`: semilla de simulacion (default: `42`)
- `--max-ticks <u64>`: cantidad maxima de ticks (default: infinito)
- `--speed <u32>`: ticks logicos por segundo (default: `30`)
- `--no-gui`: fuerza modo headless

## Features de Cargo

- `visualizer`: habilita visualizacion en tiempo real (`eframe` + `wgpu`)
- `parquet-export`: habilita exportacion Parquet (placeholder de integracion)

## Estructura del repositorio

- `src/`: motor de simulacion y sistemas ECS
- `benches/`: benchmarks con Criterion
- `docs/`: documentacion funcional y tecnica
- `design/`: arquitectura, backlog iterativo, especificacion y RFCs
- `docs/informe.tex`: informe tecnico formal del proyecto

## Resumen tecnico

```
Lenguaje:          Rust 2021 (release hardened)
ECS:               hecs
Grid:              100x100, chunks 16x16, bordes absorbentes
Paralelismo:       rayon + double-buffering
Campo:             3 canales de feromonas, difusion gaussiana + decaimiento
RNG:               rand_xoshiro (determinista por semilla/agente/tick)
Metricas:          JSON cada 60 ticks (Parquet por feature)
Tick logico:       1 tick = 1 segundo simulado
Escala objetivo:   miles de agentes concurrentes
```

## Documentacion principal

### docs/

| Archivo | Contenido |
|---|---|
| `docs/01_vision_general.md` | Motivacion, objetivos, alcance v1 y vision de largo plazo |
| `docs/02_tecnologias_stack.md` | Stack tecnologico, decisiones de librerias y entorno objetivo |
| `docs/03_arquitectura_capas.md` | Arquitectura por capas, frecuencia de sistemas y acoplamientos |
| `docs/04_modelos_biologicos.md` | Modelos de energia, roles, feromonas, SIR, cria y metricas |
| `docs/05_hoja_de_ruta.md` | Plan por fases, criterios de exito y evolucion del producto |
| `docs/informe.tex` | Informe tecnico formal del proyecto |

### design/

| Archivo | Contenido |
|---|---|
| `design/architecture.md` | Arquitectura tecnica detallada del motor |
| `design/simulation_spec.md` | Especificacion operativa de la simulacion |
| `design/backlog.md` | Flujo de desarrollo por iteraciones (M0--M15) |
| `design/agents.md` | Roles de trabajo y responsabilidades de desarrollo |
| `design/complexity_matrix.md` | Riesgos y complejidad por componente |
| `design/rfcs/` | Propuestas de cambio de arquitectura/modelo |

## Estado actual

- Motor funcional con modulos M0--M15 implementados.
- Exportacion de metricas activa para analisis post-simulacion.
- Visualizador disponible por feature flag.
- Suite de tests y benchmarks disponible en el repo.

## Guia de lectura recomendada

- Si eres nuevo: `docs/01_vision_general.md` -> `docs/03_arquitectura_capas.md`
- Si vas a implementar sistemas: `design/simulation_spec.md` -> `src/orchestrator.rs` -> `src/systems.rs`
- Si vas a planificar cambios grandes: `design/backlog.md` + `design/rfcs/`
