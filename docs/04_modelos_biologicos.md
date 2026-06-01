# BeeFlow — Modelos Biológicos y Mecánicas de Simulación

## Principio General

Cada mecánica de simulación en BeeFlow está respaldada por un modelo matemático o biológico explícito. Esto garantiza consistencia científica, permite calibración con datos empíricos y evita comportamientos "decorativos" que no afectan la dinámica real de la colonia.

---

## 1. Balance Energético Individual

Cada abeja mantiene un balance energético propio que determina su supervivencia y capacidad de acción.

### Parámetros Base

```
Almacenamiento máximo:    1.0 units
Costo metabólico basal:   0.02 units / tick
Ganancia por forrajeo:    +0.5 a +2.0 units  (según calidad del recurso)
Umbral de trofalaxia:     colony_reserve < 30%
```

### Ecuación por Tick

```
energy_t+1 = energy_t
             - metabolic_cost(global_temp)
             + forage_gain(resource_celda)     [si es recolectora]
             + trophallaxis_gain               [si colony_reserve < 30%]
```

### Muerte por Energía

Un agente muere cuando `energy ≤ 0`. No hay recuperación espontánea sin ingesta.

### Trofalaxia (Compartir Alimento)

Cuando las reservas globales caen por debajo del 30%, las abejas cercanas se transfieren energía activamente. El sistema `TrophallaxisSystem` identifica pares de agentes en la misma celda o celdas adyacentes y redistribuye energía del que tiene más hacia el que tiene menos, con una tasa configurable.

---

## 2. Asignación y Transición de Roles

Los roles no son fijos ni aleatorios. Se basan en el **Fixed-Threshold Response Model** con modulación por edad y señales químicas, derivado del modelo de umbral de respuesta de Seeley (1995).

### Roles Implementados en v1

| Rol | Función Principal |
|---|---|
| Reina | Puesta de huevos; única por colmena |
| Nodriza | Alimenta y cuida la cría; responde a feromonas de cría |
| Constructora | Produce cera y construye panales |
| Guardiana | Defiende la entrada; responde a feromonas de alarma |
| Recolectora | Sale al exterior a buscar néctar/polen |
| Zángano | Reproducción; no participa en tareas de colmena |

### Algoritmo de Transición

```
1. Calcular stimulus_level para cada tarea posible:
   stimulus = pheromone_concentration(tipo) × age_factor × health_factor

2. Para cada tarea T:
   P(adoptar T) = stimulus_T² / (stimulus_T² + threshold_T²)
   [función sigmoide — suaviza transiciones]

3. El agente adopta el rol de mayor P si supera su umbral individual.

4. Los umbrales son levemente heterogéneos entre agentes
   (variación ±10% desde seed determinista).
```

Las transiciones son **suaves y continuas**, no discretas ni instantáneas. Un agente puede cambiar de rol a lo largo de varios ticks al ir acumulando exposición a estímulos.

---

## 3. Difusión de Feromonas

Las feromonas son el principal mecanismo de comunicación y coordinación de la colonia.

### Tipos de Feromonas en v1

| Tipo | Fuente | Efecto |
|---|---|---|
| Feromona de alarma | Guardianas bajo ataque | Recluta guardianas; activa comportamiento defensivo |
| Feromona de tarea | Cría, reina, reservas bajas | Modula transición de roles |
| Feromona de atracción | Fuentes de alimento marcadas | Guía recolectoras hacia recursos |

### Modelo Matemático

```
Para cada tick y cada celda (x, y):

1. Difusión (kernel gaussiano 3×3 separable):
   φ'(x,y) = Σ w(dx,dy) × φ(x+dx, y+dy)    [kernel gaussiano normalizado]

2. Decaimiento exponencial:
   φ_t+1(x,y) = φ'(x,y) × (1 - decay_rate)
   decay_rate = 0.05 / tick

3. Deposición por agentes:
   φ_t+1(x,y) += agent_emission_rate   [si hay agente activo en celda]
```

La implementación usa **double-buffering**: el buffer de lectura y el de escritura se intercambian al final de cada tick, evitando condiciones de carrera en el procesamiento paralelo.

---

## 4. Modelo de Enfermedades (SIR Simplificado)

En v1 se implementa un modelo SIR (*Susceptible → Infected → Recovered*) adaptado a la dinámica de colonia.

### Estados

```
S  Susceptible  — puede infectarse por contacto
I  Infected     — contagioso; metabolismo aumentado (+20% costo energético)
R  Recovered    — inmune temporalmente (duración configurable)
```

### Transmisión

```
P(infección por tick) = base_rate × contact_density(celda) × susceptibility(agente)

contact_density = número de agentes I en celda actual + celdas adyacentes
susceptibility  = 1.0 por defecto; reducida por estado R (0.0) o edad baja (0.8)
base_rate       = parámetro configurable por tipo de enfermedad
```

### Impacto en la Colonia

- Los agentes infectados consumen más energía y tienen menor eficiencia de forrajeo.
- Si la infección supera un umbral de prevalencia, puede activar comportamientos de respuesta colectiva (aislamiento, limpieza).
- Sin tratamiento o recuperación suficiente, la enfermedad puede contribuir al colapso de la colonia.

### Limitaciones de v1

- Sin mutaciones ni variantes.
- Sin modelado de varroa ni nosema específicamente (genérico).
- Sin inmunidad materna transmitida a la cría.

---

## 5. Interacción con Depredadores

En v1 los depredadores son agentes del ECS con componentes propios que interactúan con el grid y con las abejas.

### Comportamiento Básico

- Los depredadores se mueven por el grid siguiendo gradientes de feromona de atracción o de forma aleatoria (configurable).
- Al entrar en contacto (misma celda o adyacente), pueden atacar abejas con una tasa de éxito probabilística.
- Las guardianas responden al depredador con feromona de alarma, reclutando refuerzos.

### Consecuencias

- Abejas atacadas pierden energía; si llegan a 0, mueren.
- La feromona de alarma generada puede desestabilizar la colonia temporalmente.

---

## 6. Ciclo de Cría

La producción de nueva cría está gobernada por variables de System Dynamics y condiciones locales.

### Proceso

```
1. La reina deposita huevos a una tasa = brood_production_rate
   brood_production_rate depende de:
   - honey_reserve  (umbral mínimo necesario)
   - global_temp    (temperaturas extremas la reducen)
   - season_phase   (primavera/verano = máxima producción)

2. Los huevos existen como agentes "cría" en celdas específicas del grid.

3. Las nodrizas los atienden: consumen energía propia y depositan alimento.

4. Tras un período de desarrollo (configurable en ticks), la cría eclosiona
   como abeja adulta con rol inicial de nodriza.
```

---

## 7. Métricas de Emergencia y Validación

BeeFlow no solo simula: mide. Las siguientes métricas permiten evaluar si el comportamiento emergente es biológicamente realista:

| Métrica | Descripción | Frecuencia |
|---|---|---|
| `population_by_role` | Distribución de población por rol | Cada 60 ticks |
| `colony_reserve` | Reservas energéticas totales | Cada 60 ticks |
| `mortality_rate` | Muertes por causa (energía, enfermedad, depredación) | Cada 60 ticks |
| `pheromone_entropy` | Entropía espacial de distribución de feromonas | Cada 60 ticks |
| `foraging_efficiency` | Ratio recursos traídos / energía gastada en forrajeo | Cada 60 ticks |
| `brood_adult_ratio` | Ratio cría activa / población adulta | Cada 60 ticks |
| `time_to_collapse` | Ticks hasta que la colonia cae por debajo de umbral crítico | Al evento |
| `sir_prevalence` | % de agentes en cada estado S/I/R | Cada 60 ticks |

Estas métricas se exportan en formato JSON/Parquet para análisis posterior.
