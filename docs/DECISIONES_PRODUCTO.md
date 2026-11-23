# MyCalibro — decisiones de producto

**Repositorio y fuente de verdad:** JavierRB35/MyCalibro, rama `main`.
**Método:** registrar las decisiones expresas del propietario; no completar decisiones pendientes por inferencia. Aplicar `AGENTS.md` y el Protocolo de desarrollo asistido por inteligencia artificial facilitado por el usuario. Cada decisión aprobada informa la planificación futura, sin representar por sí sola una implementación funcional.

## PROD-001 — Visión del producto
- **Fecha:** 2026-11-23.
- **Estado:** APROBADA por el propietario.
- **Alternativa seleccionada:** **C: plataforma integral de biblioteca, lectura e inteligencia artificial**.
- **Alcance estratégico:** evolucionar el gestor de ebooks actual a un producto integral con gestión de catálogo, formatos y dispositivos, lectura integrada y asistencia inteligente. Los alcances concretos de lectura y de IA los determinan las decisiones posteriores (en particular, PROD-004 limita el lector a la alternativa básica). El objetivo es disponer de una amplitud funcional útil, cuidando fiabilidad, experiencia de uso y mantenimiento.
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
- **Fecha de apertura y aprobación:** 2026-11-23.
- **Estado:** APROBADA por el propietario.
- **Alternativa seleccionada:** **A: lector básico integrado para EPUB y PDF**.
- **Comportamiento vinculante:** abrir y leer EPUB/PDF dentro de MyCalibro; navegar por capítulos/páginas según el formato; ofrecer preferencias básicas de visualización (tipografía y tamaño cuando el formato lo permita, modo claro/oscuro); conservar y recuperar la última posición de lectura.
- **Alcance expresamente no aprobado para el lector:** subrayados, notas, marcadores avanzados, estadísticas de lectura, OCR, lectura de PDF escaneados mediante reconocimiento de texto y consultas o resúmenes de IA integrados en la interfaz del lector. No introducirlos como funciones del lector sin una nueva decisión.
- **Coherencia con PROD-001:** la visión integral de biblioteca, lectura e IA sigue vigente. Esta elección acota el **lector** al nivel básico; no elimina la clasificación con IA ya existente ni impide debatir funcionalidades de IA separadas en módulos de biblioteca o consulta.
- **Protección de datos:** la posición de lectura y preferencias deben persistirse sin corromper el catálogo, respetar archivos vinculados/administrados (PROD-002) y conservar los originales sin alteraciones.
- **Límites:** no se ha aprobado motor de renderizado, modelo de posiciones EPUB/PDF, compatibilidad DRM, formato de almacenamiento de preferencias o detalle de UX. Son decisiones de implementación a justificar y verificar.
- **Estado de implementación:** pendiente; la aplicación actual no incorpora un lector EPUB/PDF. Esta decisión actualiza el alcance, no el código.

## PROD-005 — Alcance de la inteligencia artificial
- **Fecha de apertura:** 2026-11-23.
- **Estado:** PENDIENTE DE ELECCIÓN.
- **Pregunta:** ¿Hasta dónde debe llegar la IA de MyCalibro **fuera del lector básico aprobado en PROD-004**?

| Opción | Funcionalidades previstas | Alcance y coste |
| --- | --- | --- |
| A. IA para organizar | Clasificación temática y etiquetas, normalización y mejora de fichas bibliográficas, detección asistida de duplicados | Menor complejidad; sin análisis del contenido de los libros |
| B. IA por libro, bajo demanda | Todo A, más resumen/argumento, análisis y preguntas sobre el contenido del libro cuando pueda extraerse con permiso; respuestas fundamentadas en pasajes verificables | Requiere extracción de texto, manejo de límites y transparencia sobre qué se ha procesado; no modifica la interfaz del lector |
| C. IA para libros y biblioteca completa | Todo B, más consultas que crucen varios libros, búsqueda semántica y organización/recomendaciones de la colección | Mayor complejidad de indexación, privacidad y costes; se materializaría en un módulo de consulta separado del lector |

**Recomendación no aprobada:** B si se prioriza utilidad por libro con complejidad acotada; C si el objetivo es análisis transversal de toda la colección. En cualquier opción, solo enviar a servicios externos la información autorizada, distinguir metadatos de contenido realmente leído y evitar referencias ficticias.

**Pendiente:** elegir A, B o C. La clasificación Gemini→Groq que existe actualmente no se interpreta como prueba de las capacidades futuras.

## Pendientes de debates posteriores
- Detalle de importación puntual desde Calibre: campos, duplicados, política de copia/vínculo y recuperación (PROD-003, aprobada).
- Detalles de implementación del lector básico EPUB/PDF, navegación, visualización y última posición (PROD-004, aprobada).
- Alcance de IA para organización, libros o biblioteca completa (PROD-005, abierta) y controles de privacidad.
- Plataformas (Windows / otras), instalación y actualizaciones.
- Copia y restauración, sincronización y dispositivos.

## Historial de decisiones
| ID | Fecha | Estado | Resultado |
| --- | --- | --- | --- |
| PROD-001 | 2026-11-23 | Aprobada | Opción C: plataforma integral de biblioteca, lectura e IA |
| PROD-002 | 2026-11-23 | Aprobada | Opción C: almacenamiento híbrido (archivos administrados y vinculados) |
| PROD-003 | 2026-11-23 | Aprobada | Opción A: importación puntual sin sincronización posterior con Calibre |
| PROD-004 | 2026-11-23 | Aprobada | Opción A: lector básico EPUB/PDF, navegación, visualización y posición |
| PROD-005 | 2026-11-23 | Pendiente | Alcance de IA fuera del lector básico |
