# BeeFlow — Conceptos de Simulacion de Sistemas Aplicados

Este documento explica que conceptos clasicos de simulacion de sistemas aparecen en BeeFlow, como se modelan en la documentacion de `docs/` y `design/`, y donde se ven en el codigo de `src/`.

## 1) Simulacion hibrida (multi-paradigma)

BeeFlow no usa un solo enfoque, sino una combinacion de paradigmas:

- **Simulacion basada en agentes (ABM):** cada abeja, cria y depredador es una entidad con estado propio y reglas locales.
- **Dinamica de sistemas (System Dynamics):** variables globales agregadas (temperatura, estacion, reservas, tasa de cria) que evolucionan en otra escala temporal.
- **Modelo espacial discreto (grid cellular):** el entorno es una grilla 2D donde se propagan señales y recursos.
- **Proceso de campo continuo sobre malla:** las feromonas se modelan como concentraciones continuas que difunden y decaen.

Referencia en documentacion:

- `docs/01_vision_general.md`
- `docs/03_arquitectura_capas.md`
- `design/architecture.md`

Implementacion en codigo:

- Orquestacion por capas: `src/orchestrator.rs`
- Componentes/agentes: `src/components.rs`, `src/systems.rs`
- Variables macro: `src/system_dynamics.rs`
- Grilla y campo: `src/grid.rs`, `src/diffusion.rs`

---

## 2) Tiempo discreto y scheduler determinista

La simulacion avanza en **ticks** (pasos discretos). Cada tick aplica un orden fijo de sistemas, lo cual evita ambiguedad causal y facilita reproducibilidad.

Conceptos aplicados:

- **Reloj logico discreto** (`tick` como unidad de avance).
- **Actualizacion sincrona por etapas** (orden canonicamente definido).
- **Multitasa temporal**: no todo se recalcula en cada tick (ej. dinamica global cada 15 ticks, export de metricas cada 60).

En codigo:

- Bucle principal y `tick_once`: `src/orchestrator.rs`
- Update multitasa: `if self.tick % 15 == 0` para System Dynamics
- Export periodico: `if self.tick % 60 == 0` para metricas

Esto materializa una idea central de simulacion: **separar escalas temporales** para modelar procesos rapidos y lentos sin perder coherencia ni rendimiento.

---

## 3) ABM: interacciones locales y comportamiento emergente

En BeeFlow, las reglas son locales (por agente y vecindad), pero los patrones observados son globales (productividad, colapso, respuesta colectiva).

Conceptos de simulacion presentes:

- **Estado interno por agente** (energia, edad, rol, salud SIR, sensibilidad a feromonas).
- **Reglas de transicion de estado** (cambio de rol, progresion SIR, desarrollo de cria).
- **Interaccion local espacial** (misma celda o vecindad).
- **Emergencia**: de reglas simples locales surgen dinamicas de colonia.

En codigo:

- Estados de agentes: `src/components.rs`
- Sistemas de comportamiento: `src/systems.rs`
  - movimiento
  - energia
  - forrajeo
  - trofalaxia
  - enfermedad
  - mortalidad
  - transicion de roles
  - cria
  - depredadores

---

## 4) Estructura espacial y procesos de transporte

El entorno es una grilla 100x100 con bordes absorbentes y obstaculos en zona de borde.

Conceptos aplicados:

- **Discretizacion espacial** del dominio en celdas.
- **Condiciones de borde absorbentes** (no rebote, no toroide).
- **Vecindades 4 y 8 conectadas** para reglas de movimiento/contacto.
- **Transporte de informacion en el medio** via difusion de feromonas.

Difusion:

- Kernel gaussiano separable 3x3 (paso horizontal + vertical).
- Decaimiento exponencial por tick.
- Doble buffer para evitar efectos de lectura/escritura cruzada en el mismo paso.

En codigo:

- Estructura de grilla: `src/grid.rs`
- Difusion y decaimiento: `src/diffusion.rs`

Este bloque representa el equivalente computacional de una **PDE simplificada en malla** (difusion-reaccion discretizada).

---

## 5) Retroalimentaciones (feedback loops) y no linealidad

BeeFlow incorpora la idea de sistema dinamico con bucles de realimentacion:

- **Temperatura global -> costo metabolico**: cambia el gasto energetico de agentes.
- **Reserva de miel -> tasa de cria**: reservas altas habilitan mayor reposicion poblacional.
- **Reserva baja -> trofalaxia**: mecanismo de redistribucion de energia en crisis.
- **Estacion -> regeneracion de recursos**: disponibilidad ambiental periodica.
- **Pesticidas -> menor eficiencia / mayor desgaste**: deterioro sistemico externo.

Estas relaciones incluyen no linealidades (umbrales, clamps, funciones sinusoidales, probabilidades), lo que favorece transiciones de fase entre regimenes (estabilidad, estres, colapso).

Referencias:

- `docs/04_modelos_biologicos.md`
- `design/simulation_spec.md`
- `src/system_dynamics.rs`
- `src/systems.rs`

---

## 6) Procesos estocasticos y control de incertidumbre

La simulacion usa aleatoriedad para modelar variabilidad biologica y decisiones probabilisticas:

- contagio por probabilidad por tick
- ataques de depredador con probabilidad de exito
- adopcion de rol segun probabilidad del modelo de umbral
- variacion individual de umbrales (heterogeneidad)

Pero al mismo tiempo se conserva reproducibilidad experimental:

- **Semilla global configurable**
- **RNG determinista por agente/tick**
- tests que verifican igualdad de salidas con misma seed

En codigo:

- RNG y seeds: `src/rng.rs`, `src/config.rs`
- uso en sistemas: `src/systems.rs`, `src/orchestrator.rs`
- tests de determinismo: `src/orchestrator.rs`

Este equilibrio entre estocasticidad y repetibilidad es clave en simulacion cientifica.

---

## 7) Modelado epidemiologico acoplado (SIR)

Se integra un submodelo epidemiologico SIR dentro del ABM, acoplado al espacio y a la energia:

- estados `Susceptible -> Infected -> Recovered`
- probabilidad de infeccion dependiente de densidad de infectados en vecindad
- transiciones temporizadas por duracion de estado
- impacto funcional: infectados consumen mas energia y forrajean peor

Esto es un ejemplo de **modelo compuesto**: epidemiologia + comportamiento + recursos + espacio.

En codigo:

- estado de salud: `src/components.rs`
- dinamica SIR: `src/systems.rs` (`run_disease_system`)

---

## 8) Dinamica de roles como sistema adaptativo

El cambio de rol se basa en un **Fixed-Threshold Response Model** (inspirado en Seeley), donde la probabilidad de adoptar tarea depende de:

- intensidad de estimulo (feromona)
- edad
- estado de salud
- umbral individual (heterogeneo)

Esto implementa un concepto central de sistemas complejos: **especializacion flexible autoorganizada** sin controlador central.

En codigo:

- transicion de roles: `src/systems.rs` (`run_role_transition_system`)

---

## 9) Criterios de evento y condiciones terminales

La simulacion define eventos criticos explicitamente:

- muerte individual por energia en 0
- colapso poblacional cuando la colonia cae bajo umbral

Esto aporta condiciones de parada y de analisis de riesgo, tipicas en modelos de resiliencia.

En codigo:

- mortalidad: `src/systems.rs` (`run_mortality_system`)
- tiempo a colapso: `src/orchestrator.rs` (`time_to_collapse`)

---

## 10) Medicion, observabilidad y validacion

BeeFlow no solo ejecuta reglas; tambien instrumenta el sistema para analizar su dinamica.

Metricas clave:

- distribucion por rol
- reservas de colonia
- mortalidad por causa
- eficiencia de forrajeo
- ratio cria/adultos
- prevalencia SIR
- entropia espacial de feromonas
- tiempo a colapso

Conceptos de simulacion aplicados:

- **variables de salida/observables** para comparar escenarios
- **indicadores emergentes** (entropia, eficiencia)
- **series temporales exportables** para analisis estadistico posterior

En codigo:

- estructura/export de metricas: `src/metrics.rs`
- armado de snapshot: `src/orchestrator.rs` (`build_snapshot`)

---

## 11) Rendimiento como parte del diseno de simulacion

En BeeFlow, las decisiones de arquitectura tambien responden a principios de simulacion eficiente:

- ECS para iteraciones cache-friendly
- layout SOA en grilla
- chunking espacial
- paralelizacion con rayon
- doble buffer en hot path de feromonas
- separacion de thread de render y thread de simulacion

Esto permite escalar cantidad de agentes y duracion de runs sin alterar reglas del modelo.

Referencias:

- `docs/03_arquitectura_capas.md`
- `design/architecture.md`
- `design/complexity_matrix.md`
- `src/grid.rs`, `src/diffusion.rs`, `src/orchestrator.rs`

---

## 12) Trazabilidad spec -> arquitectura -> codigo

Una fortaleza metodologica del proyecto es la trazabilidad entre capas de definicion:

- **Verdad biologica:** `design/simulation_spec.md`
- **Verdad tecnica:** `design/architecture.md`
- **Narrativa y contexto:** `docs/`
- **Implementacion ejecutable:** `src/`

Desde la perspectiva de simulacion de sistemas, esto es importante porque permite:

- reproducir experimentos
- auditar supuestos
- detectar divergencias entre modelo conceptual e implementacion
- evolucionar el modelo de forma controlada

---

## Notas sobre coherencia y calibracion

Durante la revision se observan algunas diferencias puntuales entre ciertos parametros de la spec y valores de implementacion actual (por ejemplo, constantes energeticas o umbrales operativos). Esto no invalida el enfoque de simulacion, pero si marca una tarea tipica de proyectos de modelado: **calibrar y alinear continuamente spec y codigo**.

Documentos donde ya se explicita esta necesidad:

- `design/architecture.md` (incluye divergencias documentadas)
- `design/AGENTS.md` (regla de oro: la spec manda)

En terminos de ingenieria de simulacion, esta etapa corresponde a **verificacion y validacion (V&V)** continua.
