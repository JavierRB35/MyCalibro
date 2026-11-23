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
- **Fecha de apertura:** 2026-11-23.
- **Estado:** PENDIENTE DE ELECCIÓN; ninguna alternativa autorizada.
- **Pregunta:** ¿Los archivos de los libros se incorporarán al almacenamiento gestionado de MyCalibro, permanecerán donde están o podrán coexistir ambos modelos?

| Opción | Comportamiento | Ventaja principal | Riesgo o coste |
| --- | --- | --- | --- |
| A. Biblioteca administrada | Al importar, copiar el libro a una carpeta administrada por la app, conservando inicialmente el original | Biblioteca más portable, controlada y sencilla de respaldar | Más espacio, posibles duplicados; hace falta migración segura |
| B. Solo referencias externas | Mantener ruta al archivo existente sin copiarlo | Sin duplicar espacio y máxima compatibilidad inicial con carpetas/Calibre | Enlaces rotos si se mueven archivos; copias incompletas si se respalda solo el catálogo |
| C. Híbrida | Permitir libro administrado o vinculado, con estado explícito y selector de política de importación | Se adapta a bibliotecas de Calibre y colecciones propias; migración gradual | Más estados y reglas que probar, especialmente backups y archivos externos |

**Recomendación técnica para el debate (NO aprobada):** C, con un repositorio administrado en carpeta configurable, posibilidad de vincular archivos sin copiarlos, detección de rutas faltantes y distinción inequívoca en la copia de seguridad de archivos administrados y externos. El cambio no debe mover ni borrar originales automáticamente. Priorizar una migración que preserve el catálogo actual antes de alterar su esquema.

**Criterios que deberá concretar la opción escogida:** ubicación y configuración, qué significa «importar» por defecto, tratamiento de duplicados, qué entra en el backup/restauración, compatibilidad con colecciones Calibre, recuperación de archivos ausentes y manejo de varias copias/formatos de una misma obra.

## Pendientes de debates posteriores
- Integración con Calibre y migración de bibliotecas existentes.
- Experiencia de lectura, PDF/EPUB, anotaciones y OCR.
- IA sobre metadatos frente a contenido y controles de privacidad.
- Plataformas (Windows / otras), instalación y actualizaciones.
- Copia y restauración, sincronización y dispositivos.

## Historial de decisiones
| ID | Fecha | Estado | Resultado |
| --- | --- | --- | --- |
| PROD-001 | 2026-11-23 | Aprobada | Opción C: plataforma integral de biblioteca, lectura e IA |
| PROD-002 | 2026-11-23 | Pendiente | Modelo de almacenamiento por decidir |
