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
- **Fecha de apertura y aprobación:** 2026-11-23.
- **Estado:** APROBADA por el propietario.
- **Alternativa seleccionada:** **A: importación puntual desde Calibre**.
- **Comportamiento vinculante:** permitir importar a MyCalibro libros y metadatos de una biblioteca de Calibre existente. Una vez importados, las modificaciones posteriores en cualquiera de las aplicaciones se gestionarán **independientemente**, sin sincronización automática ni bidireccional entre los catálogos.
- **Protección de origen:** la importación no debe escribir ni borrar archivos ni registros de la biblioteca de Calibre. La estructura e integridad de la biblioteca de origen deben preservarse.
- **Compatibilidad con PROD-002:** el modelo híbrido de MyCalibro se mantiene: al importar una biblioteca de Calibre se podrán incorporar los libros como archivos administrados o referencias externas, conforme a una política de importación que queda por concretar. Una referencia externa **no** implica sincronización de metadatos ni seguimiento automático de modificaciones.
- **Límites de la decisión:** aún no están fijados los campos precisos a importar (portadas, series, etiquetas, múltiples formatos, etc.), tratamiento de duplicados, modo por defecto, errores, migración parcial y experiencia del asistente de importación. La importación repetida será otra operación puntual, no una sincronización permanente.
- **Estado de implementación:** pendiente; el código existente importa ebooks sueltos por arrastrar y soltar, pero no incorpora una biblioteca completa de Calibre.

## PROD-004 — Alcance del lector integrado
- **Fecha de apertura:** 2026-11-23.
- **Estado:** PENDIENTE DE ELECCIÓN.
- **Pregunta:** ¿Qué nivel de experiencia de lectura debe ofrecer MyCalibro, dentro de la visión integral con IA ya aprobada en PROD-001?

| Opción | Alcance previsto | Implicaciones |
| --- | --- | --- |
| A. Lector básico | Apertura y lectura de EPUB/PDF, navegación, preferencias de visualización y progreso | Menor complejidad inicial; quedan fuera anotaciones avanzadas y OCR |
| B. Lector premium | Todo A, además de marcadores, subrayados, notas, búsqueda en el texto, índice, progreso y personalización avanzada | Más valor de lectura; exige un modelo sólido de ubicaciones y persistencia |
| C. Lector avanzado + IA | Todo B, además de OCR para PDF escaneados, consultas y resúmenes contextuales con IA, referencias al contenido y accesibilidad reforzada | Máxima capacidad, pero exige separar claramente lectura, extracción de texto, OCR e IA y probar formatos/errores |

**Recomendación para el debate (no aprobada):** C, desarrollada incrementalmente: EPUB/PDF fiables, anotaciones persistentes, después OCR y capacidades IA. El acceso a archivos y el tratamiento de datos deberán respetar PROD-002, y las operaciones de IA no deben inventar citas o contenido de libros no procesados.

**Pendiente:** seleccionar A, B o C. Ninguna funcionalidad nueva de lector se declara implementada por esta elección pendiente.

## Pendientes de debates posteriores
- Detalle de importación puntual desde Calibre: campos, duplicados, política de copia/vínculo y recuperación (PROD-003, aprobada).
- Experiencia de lectura, PDF/EPUB, anotaciones y OCR (PROD-004, abierta).
- IA sobre metadatos frente a contenido y controles de privacidad.
- Plataformas (Windows / otras), instalación y actualizaciones.
- Copia y restauración, sincronización y dispositivos.

## Historial de decisiones
| ID | Fecha | Estado | Resultado |
| --- | --- | --- | --- |
| PROD-001 | 2026-11-23 | Aprobada | Opción C: plataforma integral de biblioteca, lectura e IA |
| PROD-002 | 2026-11-23 | Aprobada | Opción C: almacenamiento híbrido (archivos administrados y vinculados) |
| PROD-003 | 2026-11-23 | Aprobada | Opción A: importación puntual sin sincronización posterior con Calibre |
| PROD-004 | 2026-11-23 | Pendiente | Alcance y profundidad del lector integrado |
