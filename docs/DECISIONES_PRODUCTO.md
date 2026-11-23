# MyCalibro — decisiones de producto

**Repositorio y fuente de verdad:** JavierRB35/MyCalibro, rama `main`.
**Método:** registrar las decisiones expresas del propietario; no completar decisiones pendientes por inferencia. Aplicar `AGENTS.md` y el Protocolo de desarrollo asistido por inteligencia artificial facilitado por el usuario. Cada decisión aprobada informa la planificación futura, sin representar por sí sola una implementación funcional.

## PROD-001 — Visión del producto
- **Fecha:** 2026-11-23.
- **Estado:** APROBADA por el propietario.
- **Alternativa seleccionada:** **C: plataforma integral de biblioteca, lectura e inteligencia artificial**.
- **Alcance estratégico:** evolucionar el gestor de ebooks actual a un producto integral con gestión de catálogo, formatos y dispositivos, lectura integrada, anotaciones, búsqueda y asistencia inteligente. El objetivo es disponer de la máxima amplitud funcional útil, cuidando fiabilidad, experiencia de uso y mantenimiento.
- **Criterios vinculantes:** conservar las funcionalidades existentes; desarrollo incremental y comprobable; protección de datos de usuario; IA integrada con errores explícitos y sin funcionalidades aparentes; arquitectura proporcional al crecimiento; GitHub `main` como fuente única de verdad y rama de trabajo habitual.
- **Límites:** la dirección aprobada no determina todavía plataforma objetivo, ubicación física de archivos, sincronización, modelos IA concretos, UX, prioridades pormenorizadas ni arquitectura final; tampoco implica que los componentes futuros ya existan.
- **Contexto técnico inicial:** [Auditoría funcional 2026-11-23](AUDITORIA_FUNCIONAL_2026-11-23.md). Entre los riesgos previos a nuevas funcionalidades figuran las escrituras no atómicas del catálogo y la extensión incorrecta en ciertos envíos a lectores.
- **Implementación en esta decisión:** solo documentación; ningún cambio al código.

## PROD-002 — Modelo de almacenamiento de libros
- **Fecha de apertura y aprobación:** 2026-11-23.
- **Estado:** APROBADA por el propietario.
- **Alternativa seleccionada:** **C: sistema híbrido de almacenamiento**.
- **Comportamiento vinculante:** MyCalibro permitirá coexistir libros incorporados a una biblioteca administrada y libros vinculados en sus ubicaciones externas, sin necesidad de copiar estos últimos. La distinción deberá quedar identificable para que el usuario sepa qué archivos controla MyCalibro y cuáles dependen de otras carpetas o gestores.
- **Objetivo:** mantener compatibilidad con colecciones y carpetas existentes y, a la vez, permitir una biblioteca propia organizada, portable y recuperable. Se conservará la funcionalidad actual mientras se desarrolla la transición.
- **Seguridad e integridad:** no mover ni borrar archivos originales automáticamente; proteger el catálogo durante la migración. Detectar referencias rotas, diferenciar adecuadamente archivos administrados y externos en los procesos de recuperación y no presentar como completo un backup que omita libros vinculados.
- **Límites de la decisión:** aún no se ha aprobado el diseño de carpetas, la ubicación por defecto, el modo predeterminado al importar, políticas de duplicados, sincronización, incorporación de archivos externos en copias de seguridad, ni migración precisa de metadatos. Se definirán con criterios técnicos y decisiones posteriores.
- **Estado de implementación:** no implementado todavía como sistema híbrido: el código actual conserva únicamente referencias a rutas absolutas de archivos externos (véase auditoría). Esta aprobación solo actualiza documentación.

## PROD-003 — Integración con bibliotecas de Calibre
- **Fecha de apertura:** 2026-11-23.
- **Estado:** PENDIENTE DE ELECCIÓN.
- **Pregunta:** ¿Hasta qué grado debe interoperar MyCalibro con las bibliotecas existentes de Calibre?

| Opción | Comportamiento propuesto | Principal ventaja | Riesgos y limitaciones |
| --- | --- | --- | --- |
| A. Importación puntual | Importar libros y metadatos desde Calibre a MyCalibro; futuras modificaciones se gestionan separadamente | Menor complejidad y menor riesgo de cambios en bibliotecas Calibre | Los cambios posteriores no se sincronizan |
| B. Vinculación y consulta continua | Mostrar la biblioteca Calibre vinculada y actualizar su información de forma controlada, con acceso de solo lectura para la biblioteca de origen; gestionar cambios propios en MyCalibro | Respeta Calibre como propietario de sus datos y muestra sus actualizaciones | Las ediciones en MyCalibro no retornan automáticamente a Calibre |
| C. Importación más sincronización bidireccional controlada | Combinar importación inicial y conexión posterior, pudiendo propagar cambios entre ambos catálogos mediante operaciones verificadas, conflictos explícitos y procedimientos de recuperación | Máxima interoperabilidad si se implementa de forma fiable | Mayor complejidad; riesgo alto de sobrescribir metadatos, perder cambios o corromper bibliotecas ante escrituras simultáneas |

**Criterio técnico para el debate (no aprobado):** C aporta el máximo alcance, pero deberá implantarse por fases: importación y consulta de solo lectura primero; las escrituras hacia Calibre solo tras verificar mecanismos compatibles y estrategia de reconciliación, copias y recuperación. Nunca escribir directamente en una base de datos de Calibre de forma especulativa.

**Aspectos aún no resueltos:** tratamiento de categorías/etiquetas/series/portadas, múltiples formatos, duplicados, detección de cambios, conflictos, permisos, compatibilidad de versiones y qué biblioteca es la fuente de verdad para cada dato.

## Pendientes de debates posteriores
- Integración con Calibre y migración de bibliotecas existentes (PROD-003, abierta).
- Experiencia de lectura, PDF/EPUB, anotaciones y OCR.
- IA sobre metadatos frente a contenido y controles de privacidad.
- Plataformas (Windows / otras), instalación y actualizaciones.
- Copia y restauración, sincronización y dispositivos.

## Historial de decisiones
| ID | Fecha | Estado | Resultado |
| --- | --- | --- | --- |
| PROD-001 | 2026-11-23 | Aprobada | Opción C: plataforma integral de biblioteca, lectura e IA |
| PROD-002 | 2026-11-23 | Aprobada | Opción C: almacenamiento híbrido (archivos administrados y vinculados) |
| PROD-003 | 2026-11-23 | Pendiente | Modalidad de integración con bibliotecas de Calibre |
