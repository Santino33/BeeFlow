# BeeFlow — Visión General del Proyecto

## ¿Qué es BeeFlow?

BeeFlow es una simulación computacional híbrida de una colonia de abejas (*Apis mellifera*), diseñada para reproducir y analizar dinámicas biológicas complejas, comportamientos emergentes y procesos de resiliencia y colapso a escala de colonia. El proyecto combina múltiples paradigmas de simulación en un único motor integrado, orientado a la investigación científica reproducible y accesible desde hardware de consumo.

---

## Motivación

Las colonias de abejas son sistemas adaptativos complejos cuyo comportamiento colectivo emerge de la interacción local de miles de agentes individuales. Estudiar estos fenómenos requiere herramientas que combinen:

- Modelado a nivel de agente individual (movimiento, energía, roles, comunicación)
- Dinámica de señales distribuidas en el entorno (feromonas, recursos)
- Variables macroeconómicas de la colonia (reservas, ciclo de cría, estacionalidad)
- Perturbaciones externas (enfermedades, depredadores, pesticidas, clima)

BeeFlow busca llenar ese espacio: una plataforma de simulación científicamente informada, modular y ejecutable sin infraestructura de supercómputo.

---

## Objetivos Principales

### 1. Modelado biológico realista
Capturar la organización social de la colmena: asignación dinámica de roles (recolectora, nodriza, guardiana, constructora), comunicación química por feromonas, economía energética individual y colectiva, y ciclos de cría con desarrollo temporal explícito.

### 2. Fenómenos críticos y perturbaciones
Simular la propagación de enfermedades (modelo SIR), presión de depredadores, estrés por pesticidas y los mecanismos por los cuales una colonia se recupera o colapsa ante estas amenazas.

### 3. Investigación reproducible
Proveer un entorno parametrizable, con semillas de aleatoriedad configurables, exportación de datos estructurados y métricas de emergencia definidas formalmente (tasa de forrajeo, ratio cría/adulto, entropía espacial de feromonas).

### 4. Rendimiento accesible
Ejecutar 5,000–8,000 agentes simultáneos en hardware doméstico (CPU 6–16 núcleos, 16–32 GB RAM), con un presupuesto de rendimiento de < 80 ms por tick y capacidad de aceleración >30× el tiempo real para experimentos longitudinales.

---

## Alcance de la Versión 1 (v1)

| Incluido en v1 | Diferido para versiones futuras |
|---|---|
| Motor de simulación nativo (escritorio) | Despliegue web (limitaciones de threading) |
| Grid 100×100 (1 km² a 10 m/celda) | Grids >500×500 o >50k agentes |
| Hasta ~8,000 agentes activos | Compute GPU masivo (CUDA/WGPU) |
| Difusión de feromonas (CPU, double-buffering) | Dashboard web educativo desacoplado |
| Sistema de enfermedades SIR simplificado | Mutaciones y evolución genética |
| Exportación JSON/Parquet de métricas | Cliente remoto WebAssembly/WebGPU |
| Visualización científica ligera (egui/macroquad) | Integración con Godot (opcional v2) |

---

## Paradigmas de Simulación Integrados

BeeFlow no es una simulación de un solo tipo. Combina cuatro capas que se actualizan de forma sincronizada en cada ciclo (tick):

```
┌────────────────────────────────────────────────────┐
│              Simulation Orchestrator               │
│  Sincroniza tick, clima, difusión y métricas       │
├─────────────────┬──────────────────────────────────┤
│  ECS Agent Layer│  System Dynamics Layer            │
│  Agentes        │  Variables macroeconómicas        │
│  individuales   │  globales (reservas, estaciones)  │
├─────────────────┴──────────────────────────────────┤
│               Spatial Grid Layer                   │
│   Entorno 2D: recursos, feromonas, clima local     │
└────────────────────────────────────────────────────┘
```

Cada capa tiene su frecuencia de actualización definida para desacoplar escalas temporales (ver `03_arquitectura_capas.md`).

---

## Visión a Largo Plazo

Una vez validado el núcleo de simulación:

- **Integración GPU** para experimentos a mayor escala (grids y agentes masivos)
- **Dashboard web educativo** desacoplado del motor nativo, como cliente ligero
- **Exportación para análisis** estadístico, machine learning o publicación científica
- **Modelos de evolución** con variabilidad genética entre colonias

El objetivo final es construir una plataforma de simulación reproducible, de alto rendimiento y científicamente fundamentada, capaz de explorar resiliencia ecológica y dinámicas de colonias ante perturbaciones ambientales en tiempo real o acelerado.
