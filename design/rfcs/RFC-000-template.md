# RFC-000 — [Título del Cambio]

> Copia este archivo como `RFC-NNN-nombre-corto.md` antes de proponer un cambio.  
> Un RFC debe ser aprobado por QA y Arquitecto antes de que el Programador ECS comience a implementar.

---

## Metadatos

| Campo | Valor |
|---|---|
| RFC # | 000 |
| Título | [Título breve] |
| Autor (agente) | [Biólogo / Arquitecto / Programador ECS] |
| Fecha | YYYY-MM-DD |
| Estado | Borrador / En revisión / Aprobado / Rechazado |
| Módulo afectado | [M0–M15] |

---

## 1. Objetivo

¿Qué problema resuelve este cambio o qué capacidad añade?

---

## 2. Motivación

¿Por qué es necesario? ¿Qué ocurre si no se implementa?

---

## 3. Datos Requeridos

¿Qué datos del estado del mundo necesita este sistema para funcionar?

```
Lee de:  [componentes / variables globales / grid fields]
Escribe: [componentes / variables globales / grid fields]
```

---

## 4. Componentes Nuevos o Modificados

Listar cualquier componente que se añade, modifica o elimina.

```rust
// Ejemplo:
struct NuevoComponente {
    campo: f32,
}
```

---

## 5. Sistemas Afectados

| Sistema | Tipo de cambio | Descripción |
|---|---|---|
| XxxSystem | Nuevo / Modificado / Sin cambio | ... |

---

## 6. Cambios en simulation_spec.md

¿Qué secciones de `simulation_spec.md` deben actualizarse?

- Sección: [nombre de sección]
  - Parámetro nuevo: `nombre: valor`
  - Fórmula nueva o modificada: ...

---

## 7. Cambios en architecture.md

¿Se modifica algún contrato de componente, sistema o capa?

---

## 8. Impacto en Rendimiento

¿Cuánto overhead introduce este cambio estimativamente?

- Iteraciones adicionales por tick: ~N entidades × M operaciones
- Estimación: < X ms/tick
- Requiere benchmark nuevo: Sí / No

---

## 9. Riesgos

| Riesgo | Probabilidad | Impacto | Mitigación |
|---|---|---|---|
| [descripción] | Alta/Media/Baja | Alto/Medio/Bajo | [acción] |

---

## 10. Criterio de Aceptación

¿Cómo verificamos que este RFC está correctamente implementado?

- [ ] Test específico: ...
- [ ] Métrica esperada: ...
- [ ] Comportamiento emergente observable: ...

---

## 11. Preguntas Abiertas

Lista de decisiones no resueltas que QA o Arquitecto deben responder antes de aprobar.

1. ...
2. ...

---

## Historial de Revisión

| Fecha | Revisor | Comentario |
|---|---|---|
| YYYY-MM-DD | QA | ... |
| YYYY-MM-DD | Arquitecto | ... |
