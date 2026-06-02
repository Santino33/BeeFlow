# BeeFlow — Guía de la Interfaz Gráfica

Esta guía explica qué significa cada elemento visible en la ventana del simulador y cómo interpretarlo para entender el estado de la colonia.

---

## Distribución de la pantalla

```
┌──────────────────────────── TOPBAR ─────────────────────────────────────┐
│  B BeeFlow  │  ▶  ■   ×1  ×5  ×30  ×100  │  Tick 000012  00:00:06  ● RUNNING │
├─────────────┬───────────────────────────────┬───────────────────────────┤
│             │                               │                           │
│   PANEL     │        GRID CENTRAL           │    PANEL DERECHO          │
│  IZQUIERDO  │    (mapa de feromonas         │  (canal activo + stats)   │
│  (métricas) │     + agentes)                │                           │
│             │                               │                           │
├─────────────┴───────────────────────────────┴───────────────────────────┤
│       BARRA INFERIOR — 5 tarjetas con sparklines de historial           │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 1. Topbar (barra superior)

La topbar muestra el estado de ejecución de la simulación en todo momento.

### Controles de reproducción

| Botón | Acción |
|-------|--------|
| ▶ | Iniciar / reanudar la simulación |
| ■ | Detener la simulación |
| ×1 / ×5 / ×30 / ×100 | Velocidad de simulación (ticks por segundo) |

A mayor velocidad, los fenómenos ocurren más rápido pero la animación del mapa puede verse menos fluida.

### Contador de ticks y tiempo simulado

```
Tick     1 234
00:10:17
```

- **Tick**: unidad mínima de tiempo de la simulación. En cada tick, todas las abejas actúan una vez (movimiento, alimentación, enfermedad, etc.).
- **Tiempo HH:MM:SS**: tiempo biológico equivalente asumiendo **0.5 segundos reales por tick** (aprox. 1 minuto simulado cada 120 ticks).

### Badge de estado

| Color | Texto | Significa |
|-------|-------|-----------|
| Verde ● | RUNNING | La simulación está corriendo |
| Gris ● | WAITING | Esperando el primer frame del motor |

---

## 2. Panel izquierdo — Métricas de la colonia

Organizado en secciones colapsables. Haz clic en el encabezado de cada sección para expandirla o contraerla.

### POBLACIÓN

El número grande en ámbar es el **total de abejas vivas** en ese tick.

Debajo hay una barra horizontal por cada rol:

| Rol | Función biológica |
|-----|-------------------|
| **Reina** | Única. Pone huevos y regula la dinámica reproductiva. Si muere, la colonia colapsa. |
| **Nodrizas** | Cuidan la cría (larvas y pupas). Mantienen el brood_adult_ratio. |
| **Forrajeras** | Salen de la colmena a recolectar néctar y polen. Son la principal fuente de ingreso de energía. |
| **Guardianas** | Defienden la entrada de la colmena de los depredadores. |
| **Constructoras** | Amplían y reparan las celdas de la colmena. |
| **Zánganos** | Machos. No forrajean ni defienden. Su presencia es relevante en época reproductiva. |

**Cómo leer las barras:** el fill de cada barra es proporcional al total de la población. Si la barra de Forrajeras está al 40%, significa que el 40% de todas las abejas son forrajeras en ese momento.

> Una distribución sana suele tener más Nodrizas que cualquier otro rol, seguido de Forrajeras. Una colonia con demasiadas Guardianas y pocas Forrajeras podría estar bajo amenaza de depredador.

### SALUD (SIR)

Muestra la distribución epidemiológica de la colonia usando el modelo **SIR** (Susceptible – Infectado – Recuperado).

```
      ╭────╮
     /  I%  \      ← porcentaje infectado en el centro
    │  donut  │
     \       /
      ╰────╯

● S  Susceptibles (verde)
● I  Infectados   (rojo)
● R  Recuperados  (azul)
```

| Estado | Color | Significado |
|--------|-------|-------------|
| **S** Susceptible | Verde | Abeja sana, puede infectarse si contacta a un infectado |
| **I** Infectado | Rojo | Abeja enferma. Pierde energía extra por tick y puede contagiar a vecinas |
| **R** Recuperado | Azul | Abeja que superó la enfermedad. Temporalmente inmune |

El porcentaje en el centro del donut siempre muestra **%I** (infectados), el indicador más crítico. Si supera el 30%, la colonia entra en riesgo de colapso por enfermedad.

> Los tres arcos del donut suman siempre el 100% de la población. Si un arco no se ve, ese estado tiene 0 individuos.

### ENERGÍA

Dos barras de progreso que muestran la salud metabólica de la colonia:

| Barra | Qué mide |
|-------|----------|
| **Reserva miel** | `colony_reserve` — fracción del almacén global de miel (0–100%). Baja cuando las abejas consumen y sube cuando las forrajeras traen néctar. Si llega a 0%, las abejas mueren de hambre. |
| **Ef. forrajeo** | `foraging_efficiency` — qué tan bien están recolectando las forrajeras en este momento. Depende de cuántas fuentes de alimento hay disponibles y qué tan cerca están. |

> Una **reserva miel alta** con **eficiencia de forrajeo baja** indica que la colonia vive de sus ahorros. Es una señal de alerta a mediano plazo.

### MORTALIDAD

Muestra cuántas abejas murieron en el último período de exportación (cada 60 ticks), desglosado por causa:

| Causa | Explicación |
|-------|-------------|
| **Energía** | Murieron porque su energía individual llegó a 0 (hambre o gasto metabólico) |
| **Enfermedad** | Murieron estando en estado I (infectado) por deterioro progresivo |
| **Depredación** | Fueron eliminadas por un agente `Predator` que entró en su celda |

### Alerta de colapso

Si aparece un banner rojo **⚠ COLAPSO en tick N**, la simulación detectó que la población cayó por debajo del umbral crítico (50 individuos) por primera vez en el tick N. La colonia sigue corriendo pero se considera en fase terminal.

---

## 3. Grid central — Mapa de la colonia

El mapa muestra una grilla de 100×100 celdas. Cada celda representa una porción del espacio físico que rodea la colmena.

### Heatmap de feromonas

El fondo del mapa es un heatmap que colorea cada celda según la concentración de feromona del canal activo (seleccionado en el panel derecho).

| Canal | Color | Qué indica cuando brilla |
|-------|-------|--------------------------|
| **Atracción** | Negro → Ámbar | Ruta de feromona de atracción: las forrajeras la depositan al volver con comida para guiar a otras hacia las fuentes |
| **Alarma** | Negro → Rojo | Señal de peligro activa: las guardianas la emiten cerca de un depredador. Una mancha roja indica amenaza en esa zona |
| **Tarea** | Negro → Verde | Feromona de coordinación de tareas internas (reparación, cuidado de cría). Concentrada cerca de la colmena central |

Las zonas más brillantes tienen mayor concentración de esa feromona. Las zonas oscuras están vacías o no han sido visitadas recientemente.

> Las feromonas se **difunden** gradualmente a celdas vecinas y se **degradan** con el tiempo. Un rastro brillante que no se renueva irá desapareciendo.

### Puntos de agentes

Sobre el heatmap aparecen puntos pequeños de colores que representan abejas individuales:

| Color del punto | Rol |
|-----------------|-----|
| Amarillo brillante | Reina |
| Verde | Nodriza |
| Azul | Constructora |
| Rojo | Guardiana |
| Naranja | Forrajera |
| Gris | Zángano |
| Magenta | Depredador (agente externo, no es abeja) |

Los puntos se **interpolan suavemente** entre ticks para que el movimiento parezca continuo aunque la simulación avance en pasos discretos.

> Si ves un punto magenta cerca de la colmena y muchas manchas rojas en el heatmap de Alarma, significa que las guardianas ya detectaron al depredador.

---

## 4. Panel derecho — Canal y estadísticas

### Selector de canal de feromonas

Tres botones que cambian qué canal se muestra en el heatmap central:

- **Alarma** — útil para detectar amenazas activas
- **Tarea** — muestra coordinación interna de la colmena
- **Atracción** — muestra las rutas de recolección activas

El botón del canal activo aparece resaltado con el color correspondiente.

### AMBIENTE

Valores ambientales del tick actual que afectan el comportamiento de la colonia:

| Campo | Significado |
|-------|-------------|
| **Temperatura** | Temperatura global en °C. Temperaturas extremas (< 5°C o > 35°C) aumentan el costo metabólico de todas las abejas |
| **Estación** | Fase del ciclo estacional (0.0 = inicio de primavera, 0.25 = verano, 0.5 = otoño, 0.75 = invierno). Controla la tasa de cría y la disponibilidad de recursos |
| **Tasa cría** | Qué tan rápido se producen nuevas larvas este tick. Depende de la estación y de la reserva de miel |
| **Cría/adultos** | Ratio entre individuos en fase de cría (huevos + larvas + pupas) y abejas adultas. Un valor alto significa que hay mucha cría que alimentar; un valor bajo puede indicar problemas reproductivos |

### COLONIA

| Campo | Significado |
|-------|-------------|
| **Reserva miel** | Igual que la barra del panel izquierdo, aquí en valor numérico exacto (0.0–1.0) |
| **Ef. forrajeo** | Eficiencia de recolección en este momento (0.0–1.0) |
| **Entropía fen.** | Entropía de Shannon del campo de feromonas. Valor alto = feromonas bien distribuidas por el mapa. Valor bajo = feromonas concentradas en pocos puntos o casi inexistentes |

---

## 5. Barra inferior — Historial de métricas

Cinco tarjetas que muestran el valor actual de cada métrica **más una gráfica de tendencia (sparkline)** de los últimos 60 ticks.

```
┌────────────────┐
│  1 234      ↑  │  ← valor actual + flecha de tendencia
│  Población     │  ← etiqueta
│ ╱‾╲___╱‾╲__╱  │  ← sparkline (historial)
└────────────────┘
```

### Flecha de tendencia

| Símbolo | Color | Significa |
|---------|-------|-----------|
| ↑ | Rojo | La métrica está subiendo respecto al promedio reciente |
| ↓ | Verde | La métrica está bajando |
| → | Gris | Estable (variación < 1.5%) |

> La dirección "buena" o "mala" depende del contexto: ↑ en Población es bueno, pero ↑ en Infectados o Muertes/tick es una señal de alerta.

### Las 5 métricas

| Tarjeta | Qué mide | Alerta si… |
|---------|----------|------------|
| **Población** | Total de abejas vivas | Baja sostenida hacia < 100 |
| **Muertes/tick** | Suma de muertes por energía + enfermedad + depredación en el último tick | Valor alto y creciente |
| **Reserva miel** | `colony_reserve` en porcentaje | Cae por debajo de 20% |
| **Forrajeo** | `foraging_efficiency` en porcentaje | Se mantiene bajo (< 10%) durante muchos ticks seguidos |
| **Infectados** | Porcentaje de la población en estado I | Supera 25–30% |

El sparkline permite ver de un vistazo si la colonia está en tendencia estable, de crecimiento o en declive, sin necesidad de revisar los archivos JSON de métricas exportadas.

---

## Lectura rápida del estado de la colonia

Para evaluar la salud de una simulación en ejecución, revisa estos cuatro indicadores en orden:

1. **Reserva miel** (panel izquierdo o barra inferior) — si está en rojo o por debajo de 20%, todo lo demás empeorará rápido.
2. **% Infectados** (centro del donut SIR) — si supera 30%, la colonia puede entrar en cascada de muertes.
3. **Tendencia de Población** (sparkline) — una línea plana o descendente sostenida indica problema estructural.
4. **Heatmap de Alarma** — si hay manchas rojas grandes cerca del centro (colmena), hay un depredador activo que está aumentando la mortalidad.
