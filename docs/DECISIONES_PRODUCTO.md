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
- **Fecha de apertura y aprobación:** 2026-11-23.
- **Estado:** APROBADA por el propietario.
- **Alternativa seleccionada:** **B: IA para organizar la biblioteca y analizar cada libro, bajo demanda**.
- **Comportamiento vinculante (organización):** conservar y ampliar la clasificación por temas y etiquetas, normalización y enriquecimiento de metadatos y detección asistida de duplicados.
- **Comportamiento vinculante (por libro):** proporcionar funciones separadas del lector para obtener argumento/resumen, realizar análisis y formular preguntas sobre un libro cuando exista contenido disponible para procesar de manera autorizada. Identificar cuándo la respuesta se basa solo en metadatos y cuándo se ha examinado el texto; fundamentar las afirmaciones sobre contenido en pasajes verificables si se dispone de ellos.
- **Límites funcionales:** no se aprueban por PROD-005 consultas sobre varios libros a la vez, búsqueda semántica global, análisis transversal de toda la biblioteca ni una interfaz de IA dentro del lector básico de PROD-004.
- **Privacidad y seguridad:** el tratamiento del texto deberá respetar el consentimiento revocable por libro aprobado en PROD-006; no enviar automáticamente libros completos a servicios externos, no inventar citas y gestionar los errores de extracción/red con transparencia.
- **Coherencia con el producto existente:** el código actual ya contiene clasificación por metadatos con Gemini y alternativa Groq; eso no equivale a tener implementados resúmenes, argumentarios ni preguntas fundamentadas en el texto.
- **Estado de implementación:** decisión documental; las funcionalidades adicionales quedan pendientes de diseño, ejecución y verificación.

## PROD-006 — Privacidad y tratamiento del contenido con IA
- **Fecha de apertura y aprobación:** 2026-11-23.
- **Estado:** APROBADA por el propietario.
- **Alternativa seleccionada:** **B: consentimiento por libro, revocable**.
- **Comportamiento vinculante:** antes de transmitir contenido textual de un libro a un servicio de IA externo para analizarlo, resumirlo o responder preguntas, MyCalibro deberá obtener autorización expresa **para ese libro**. Una vez concedida, podrá reutilizar el permiso en consultas posteriores sobre ese mismo libro, sin interrumpir cada operación con una nueva confirmación.
- **Control del permiso:** el usuario podrá revisar y revocar la autorización por libro. Tras revocarla, no podrán producirse nuevos envíos de contenido de ese libro amparados en el consentimiento anterior. La revocación no implica que datos ya tratados por un proveedor puedan retirarse automáticamente de sus sistemas.
- **Información transparente:** la interfaz deberá indicar qué proveedor externo se usará (incluido cualquier proveedor de respaldo), qué categoría de información se transmite y con qué finalidad, antes de obtener el permiso y en las consultas pertinentes. Un cambio relevante de alcance de los datos o destinatarios requerirá revisar la autorización.
- **Minimización:** no enviar automáticamente el ebook completo cuando basten fragmentos y nunca asumir que un documento puede transmitirse por el mero hecho de estar en la biblioteca. Cuando no haya permiso para enviar el contenido, no simular un análisis de texto; distinguir claramente las respuestas basadas solo en metadatos.
- **Persistencia e integridad:** conservar los permisos de manera segura y vinculados a un identificador estable del libro, sin añadir secretos ni contenido privado a GitHub; contemplar importaciones, duplicados, eliminación y recuperación de copias de seguridad. La política de retención local y los detalles de implementación quedan pendientes de definición técnica.
- **Alcance y compatibilidad:** afecta al **contenido de los libros** en las funciones de PROD-005; no equivale a prohibir la clasificación existente basada en metadatos, aunque su uso de servicios externos y su transparencia deberán verificarse. Mantener separado el lector básico establecido en PROD-004.
- **Estado de implementación:** pendiente. Esta decisión documental no implica que los permisos o controles de privacidad ya existan en la app.

## PROD-007 — Plataformas de destino y ubicación de la biblioteca
- **Fecha de aprobación:** 2026-11-23.
- **Estado:** APROBADA, con alcance corregido explícitamente por el propietario.
- **Alternativa seleccionada:** **A: MyCalibro exclusivamente para Windows**.
- **Comportamiento vinculante:** interfaz, ejecución y distribución de MyCalibro como aplicación de escritorio Windows. No desarrollar una versión móvil ni integraciones entre aplicaciones como parte de este proyecto.
- **Almacenamiento:** MyCalibro deberá permitir utilizar una biblioteca ubicada en una **carpeta elegida por el usuario**, incluida una carpeta sincronizada mediante el cliente de escritorio de **Google Drive**. No se requiere implementar conexión directa a la API de Drive, autorización OAuth, autenticación, acceso remoto propio ni protocolos de intercambio con otras aplicaciones.
- **Compatibilidad con PROD-002:** mantener el modelo híbrido, diferenciando claramente archivos gestionados en la carpeta de biblioteca y referencias a archivos externos. Las rutas deben gestionarse de forma fiable si se mueve la carpeta o se cambia su ubicación.
- **Compatibilidad con PROD-003:** la importación desde Calibre es puntual, independiente y sin sincronización posterior con Calibre.
- **Riesgos técnicos a controlar:** evitar corrupción por escrituras parciales o solapadas, enlaces rotos, acceso al catálogo cuando la carpeta no esté disponible o existan copias/sincronizaciones pendientes. La sincronización de carpetas por terceros no debe considerarse una copia de seguridad verificada.
- **Rectificación del debate:** se elimina expresamente el diseño de conexión con otros productos y se cierra sin elección la anterior pregunta de interoperabilidad (anterior propuesta de PROD-008). No queda ningún requisito de integración entre aplicaciones vigente.
- **Estado de implementación:** la elección Windows está alineada con el código actual; la selección y traslado seguro de ubicación de biblioteca, incluido el uso de una carpeta sincronizada, están pendientes de implementar y verificar.

## PROD-008 — Alcance de las copias de seguridad y restauración
- **Fecha de apertura y aprobación:** 2026-11-23.
- **Estado:** APROBADA por el propietario.
- **Alternativa seleccionada:** **C: copia de seguridad completa y configurable**.
- **Contenido vinculante:** incluir en el respaldo el catálogo, fichas bibliográficas, metadatos, etiquetas, colecciones, preferencias, posiciones de lectura y demás datos persistidos relevantes. Incluir los archivos de libros administrados por MyCalibro y permitir al usuario **optar también por copiar los libros vinculados externos que sean accesibles**.
- **Control del alcance:** el usuario deberá conocer y configurar qué categorías y ubicaciones se incluyen. Una copia que omita archivos externos no deberá anunciarse como copia completa de todos los libros. Registrar claramente exclusiones, archivos inaccesibles y errores parciales.
- **Integridad y recuperación:** usar un manifiesto de contenido e identificadores, verificar la integridad de datos y archivos respaldados, detectar archivos faltantes o corruptos y ofrecer restauración segura y verificable. No sobrescribir ni borrar archivos originales, ni sustituir un catálogo válido con uno incompleto o ilegible, sin un procedimiento explícito y seguro.
- **Modelo híbrido y Drive:** preservar la diferencia entre libros administrados y referencias externas (PROD-002), y contemplar restauración a ubicaciones distintas y recuperación de rutas. Una carpeta sincronizada mediante Drive (PROD-007) **no equivale** por sí sola a una copia de seguridad verificada.
- **Aspectos técnicos no cerrados:** formato del contenedor (ZIP u otro), frecuencia y automatización, destino por defecto, retención/versiones, cifrado, política exacta de sustitución, espacio requerido, deduplicación y tratamiento de respaldos simultáneos. No inferirlos de la aprobación de la opción C.
- **Estado de implementación:** pendiente; la aplicación actual carece de una función operativa de backup/restauración. Esta decisión es exclusivamente documental.

## PROD-009 — Organización física de los libros administrados
- **Fecha de apertura:** 2026-11-23.
- **Estado:** PENDIENTE DE ELECCIÓN.
- **Pregunta:** ¿Cómo debe organizar MyCalibro los archivos que administra dentro de la carpeta de biblioteca seleccionada por el usuario?

| Opción | Funcionamiento | Ventajas y costes |
| --- | --- | --- |
| A. Conservar estructura y nombres | Mantener, dentro de la biblioteca administrada, los nombres y subcarpetas de origen cuando sea posible; el catálogo ofrece organización lógica | Menor intervención sobre archivos, pero nombres heterogéneos y potenciales conflictos |
| B. Organización automática | Al incorporar un libro administrado, ordenarlo en carpetas por autor/obra y con nombres coherentes, preservando el original externo | Biblioteca física ordenada y navegable, pero requiere resolver colisiones, cambios de metadatos y nombres |
| C. Organización configurable | Ofrecer una estructura administrada ordenada o conservar nombres/estructura según una política configurable; el catálogo maneja etiquetas/colecciones sin depender de la ruta física | Mayor flexibilidad y compatibilidad con bibliotecas existentes, con más reglas de migración y pruebas |

**Recomendación técnica (no aprobada):** C con un identificador estable por libro y con carpetas configurables. Los libros solo **vinculados externamente** nunca se reorganizarán ni renombrarán de forma automática. Cualquier cambio de política o traslado deberá ofrecer previsualización, evitar colisiones y preservar datos; no comprometer las copias de seguridad de PROD-008.

**Nota:** esta decisión se refiere a la organización física del almacenamiento administrado, no al diseño de colecciones o filtros en la interfaz; esos aspectos se concretarán posteriormente.

## Pendientes de debates posteriores
- Detalle de importación puntual desde Calibre: campos, duplicados, política de copia/vínculo y recuperación (PROD-003, aprobada).
- Detalles de implementación del lector básico EPUB/PDF, navegación, visualización y última posición (PROD-004, aprobada).
- IA para organización y análisis de cada libro (PROD-005, aprobada); consentimiento revocable por libro para envío de contenido (PROD-006, aprobada).
- MyCalibro exclusivamente Windows y biblioteca en carpeta elegida (opcionalmente sincronizada con Drive) según PROD-007; copia completa y configurable aprobada en PROD-008; instalación y actualizaciones.
- Organización física de libros administrados (PROD-009, abierta), políticas de respaldo y restauración, manejo de carpetas sincronizadas por terceros y compatibilidad con dispositivos de lectura.

## Historial de decisiones
| ID | Fecha | Estado | Resultado |
| --- | --- | --- | --- |
| PROD-001 | 2026-11-23 | Aprobada | Opción C: plataforma integral de biblioteca, lectura e IA |
| PROD-002 | 2026-11-23 | Aprobada | Opción C: almacenamiento híbrido (archivos administrados y vinculados) |
| PROD-003 | 2026-11-23 | Aprobada | Opción A: importación puntual sin sincronización posterior con Calibre |
| PROD-004 | 2026-11-23 | Aprobada | Opción A: lector básico EPUB/PDF, navegación, visualización y posición |
| PROD-005 | 2026-11-23 | Aprobada | Opción B: IA para organización y análisis por libro bajo demanda |
| PROD-006 | 2026-11-23 | Aprobada | Opción B: consentimiento por libro, revocable, antes de enviar texto a IA externa |
| PROD-007 | 2026-11-23 | Aprobada, corregida | Opción A: MyCalibro solo Windows, con biblioteca en carpeta configurable, incluso sincronizada con Drive; sin integración con otras aplicaciones |
| PROD-008 | 2026-11-23 | Aprobada | Opción C: copia de seguridad completa y configurable con integridad y restauración segura |
| PROD-009 | 2026-11-23 | Pendiente | Organización física de los archivos administrados en la biblioteca |
