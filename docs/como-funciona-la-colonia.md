# Cómo funciona la colonia de abejas en BeeFlow

Este documento explica, en términos simples, qué pasa dentro de la simulación: quiénes son los personajes, cómo viven, cómo se organizan y qué puede hacerlos prosperar o colapsar.

---

## El mundo donde viven

La simulación ocurre en una cuadrícula de 100 × 100 celdas. Piénsalo como un tablero de ajedrez muy grande. En el centro exacto está la colmena, el hogar de todas las abejas. Los bordes del tablero son zonas prohibidas, ninguna abeja puede entrar ahí.

Dispersas por el tablero hay **tres fuentes de alimento** (flores/campos) ubicadas a cierta distancia de la colmena. Cada fuente tiene un área de influencia circular donde las abejas pueden recolectar néctar. Estas fuentes se van regenerando con el tiempo, pero más rápido o más lento dependiendo de la estación del año.

---

## El tiempo y las estaciones

El tiempo avanza en pasos llamados *ticks*. Un año completo de simulación dura unos 86 400 ticks.

Hay cuatro estaciones que se van sucediendo de forma cíclica:

- **Verano**: Es la época de abundancia. Hay mucho néctar disponible, las flores se regeneran rápido y la reina pone más huevos. La colonia puede crecer.
- **Otoño / Primavera**: Transiciones. Menos actividad que el verano pero la colonia sigue funcionando.
- **Invierno**: Las flores no producen néctar. La colonia no puede crecer (no hay cría nueva) y depende completamente de la miel que haya acumulado.

La temperatura también cambia con las estaciones: puede ir de −5 °C en invierno hasta 45 °C en verano. El calor extremo hace que las abejas gasten más energía simplemente para mantenerse vivas.

---

## Los habitantes de la colmena

Dentro de la colmena conviven distintos tipos de abejas, cada una con un rol diferente.

### La reina

Hay exactamente **una reina** en toda la simulación. Está fija en el centro de la colmena y nunca se mueve. Su única función es poner huevos, pero no lo hace de forma constante: pone más cuando hay mucha miel almacenada y es verano, y deja de poner completamente en invierno o cuando la colonia está en apuros.

### Las obreras (trabajadoras)

Las obreras son la mayoría de la colonia y pueden desempeñar cuatro roles distintos a lo largo de su vida:

- **Nodrizas**: Cuidan a las larvas. Se quedan cerca de la cría y la alimentan. Son las más jóvenes.
- **Constructoras**: Mantienen y reparan la colmena.
- **Guardianas**: Defienden la entrada de la colmena de amenazas externas (depredadores).
- **Recolectoras**: Salen a buscar néctar a las fuentes de alimento y lo traen de vuelta.

Una misma abeja puede cambiar de rol durante su vida, dependiendo de lo que la colonia necesite en ese momento.

### Los zánganos

Los zánganos son los machos. No trabajan, no recolectan, no cuidan cría. Existen solo con fines reproductivos. Su desventaja es que gastan muchísima más energía que cualquier obrera, por lo que son una carga para la colonia en tiempos difíciles.

### La cría

La cría no son abejas adultas todavía. Pasa por tres etapas:

1. **Huevo**: Recién puesto por la reina. Dura un tiempo sin hacer nada.
2. **Larva**: Activa y hambrienta. Necesita que haya nodrizas cerca para sobrevivir. Si ninguna nodriza la atiende, su salud baja hasta que muere.
3. **Pupa**: Etapa de transformación silenciosa. No necesita atención. Al final de esta etapa, emerge una nueva abeja adulta (siempre como nodriza).

---

## Cómo se organizan sin un jefe

Algo fascinante de las abejas reales, y que esta simulación reproduce, es que no hay nadie dando órdenes. La organización surge sola a partir de señales químicas llamadas **feromonas**.

Hay tres tipos de feromonas en el tablero:

- **Feromona de alarma**: La emiten las guardianas cuando detectan un depredador cerca. Atrae a más guardianas hacia la amenaza.
- **Feromona de tarea**: La emiten las larvas para "pedir" que una nodriza venga a cuidarlas.
- **Feromona de atracción**: La emiten las recolectoras cuando encuentran una buena fuente de néctar. Guía a otras recolectoras hacia ese lugar.

Las feromonas se difunden por el tablero (como un olor que se esparce) y con el tiempo se disipan. Cada abeja "huele" las feromonas a su alrededor y decide hacia dónde moverse o si cambia de rol.

Cada abeja también tiene una **sensibilidad individual** distinta a estas feromonas: algunas responden con más facilidad a ciertas señales que otras. Esto hace que la colonia tenga diversidad de comportamientos, lo cual es más realista y más estable.

---

## Cómo cambian de rol las obreras

Una obrera no tiene un rol fijo para toda su vida. Cambia de rol según lo que la colonia necesite, siguiendo esta lógica:

- Si hay muchas larvas sin atender y la feromona de tarea es alta, las abejas jóvenes tienen más probabilidades de volverse nodrizas.
- Si hay una amenaza y la feromona de alarma es alta, algunas abejas se vuelven guardianas.
- Si hay buenas fuentes de néctar marcadas con feromona de atracción, abejas de mediana edad se convierten en recolectoras.

También influye la edad: las abejas muy jóvenes son mejores nodrizas; las de mediana edad, mejores constructoras y guardianas; las más viejas, mejores recolectoras (porque ya conocen el mundo exterior, por así decir).

La salud también importa: una abeja enferma tiene menos probabilidades de cambiar de rol y es menos eficiente en lo que hace.

---

## La energía y la miel

Cada abeja tiene una **barra de energía** que va de 0 a 1. Si llega a cero, la abeja muere.

Todas las abejas gastan energía solo por existir (el gasto basal del cuerpo). Los zánganos gastan mucho más que las obreras. El calor y las enfermedades también aumentan el gasto.

Las recolectoras son las únicas que pueden traer energía nueva al sistema: recogen néctar en las fuentes de alimento y lo depositan en la colmena como **miel**. Esta reserva de miel es el "banco de alimentos" de la colonia.

Cuando una abeja tiene poca energía, puede alimentarse directamente de la reserva de miel. Si la reserva se agota, las abejas empiezan a morir de hambre.

Hay otro mecanismo de supervivencia llamado **trofalaxia**: cuando la miel escasea, las abejas que tienen más energía la comparten con las que tienen menos, simplemente estando cerca unas de otras. Es una forma de redistribuir recursos en momentos de crisis.

---

## Los depredadores

La simulación puede incluir depredadores que merodean por el tablero buscando abejas. Se mueven hacia donde huelen más feromona de atracción (donde hay más actividad de recolección). Si encuentran una abeja, la atacan y le quitan energía. Si la abeja queda sin energía, muere.

Las guardianas no pueden matar al depredador directamente, pero emiten feromona de alarma al detectarlo, lo que alerta a otras guardianas para que acudan al lugar.

---

## Las enfermedades

La colonia puede sufrir una enfermedad que se propaga entre abejas que están cerca unas de otras. El modelo sigue la lógica clásica de contagio:

- **Susceptible**: Abeja sana que puede contagiarse.
- **Infectada**: Abeja enferma. Gasta más energía, recolecta menos néctar y tiene menos probabilidades de cambiar de rol. Puede contagiar a abejas cercanas.
- **Recuperada**: Abeja que superó la enfermedad. Tiene cierta resistencia temporal, pero con el tiempo vuelve a ser susceptible.

Las abejas más jóvenes son más vulnerables al contagio. Si la colonia está muy apretada y hay muchas infectadas, la enfermedad puede extenderse muy rápido.

---

## Los pesticidas

El entorno puede tener un nivel de **presión de pesticidas** configurable. Esto afecta principalmente a las recolectoras, que son las que salen al exterior. Los pesticidas les hacen gastar más energía en cada viaje, lo que reduce la eficiencia de la recolección y puede estresar a la colonia si la presión es alta.

---

## Qué puede hacer colapsar a la colonia

La colonia colapsa cuando su población cae por debajo de 50 abejas. Esto puede pasar por varias razones, que muchas veces se combinan:

- **Hambre**: Si la reserva de miel se agota (por invierno largo, pocas recolectoras, o pesticidas altos), las abejas mueren en cadena.
- **Enfermedad**: Una epidemia en una colonia densa puede matar a muchas abejas rápidamente.
- **Depredación excesiva**: Si hay muchos depredadores y pocas guardianas, las pérdidas se acumulan.
- **Falta de cría**: Si la reina deja de poner (por falta de miel o invierno), la población envejece y muere sin reemplazos.

---

## Qué mide la simulación

Cada cierto tiempo, la simulación guarda un registro con los siguientes datos:

- Cuántas abejas hay de cada tipo (reina, nodrizas, constructoras, guardianas, recolectoras, zánganos)
- Cuánta miel hay en reserva
- Cuántas abejas murieron y por qué causa (hambre, enfermedad, depredadores)
- Qué tan eficiente está siendo la recolección de néctar
- Cuánta cría hay en relación a los adultos
- Qué porcentaje de la colonia está sana, enferma o recuperada
- Qué tan distribuidas están las feromonas por el tablero (una medida de actividad)
- La temperatura actual y la fase de la estación
- En qué momento colapsó la colonia (si es que colapsó)

Estos datos permiten analizar cómo distintas condiciones (más pesticidas, más depredadores, menos reserva inicial) afectan la supervivencia de la colonia.

---

## El ciclo de un día en la colmena (resumen)

Cada paso de la simulación ocurre en este orden:

1. Se actualiza el clima (temperatura, estación, tasa de puesta de huevos)
2. Las feromonas se difunden y se disipan un poco
3. Las flores regeneran algo de néctar
4. Las abejas se mueven según las feromonas y sus roles
5. Todas las abejas envejecen un poco
6. Todas las abejas gastan energía
7. Las recolectoras recogen néctar o lo depositan en la colmena
8. Las abejas hambrientas se alimentan de la reserva
9. Las abejas comparten energía entre sí si la miel escasea
10. La enfermedad se propaga entre abejas cercanas
11. Las abejas sin energía mueren
12. Las abejas cambian de rol si las señales lo indican
13. La reina pone nuevos huevos; los huevos y larvas avanzan en su desarrollo; las pupas emergen como adultas
14. Los depredadores se mueven y atacan
15. Se verifica si la colonia colapsó
16. Se guardan las métricas (cada 60 pasos)
