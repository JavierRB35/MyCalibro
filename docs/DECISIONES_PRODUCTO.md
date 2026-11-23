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

## PROD-007 — Plataformas de destino e integración con BibliotecaAI
- **Fecha de apertura y aprobación:** 2026-11-23.
- **Estado:** APROBADA por el propietario.
- **Alternativa seleccionada:** **A: MyCalibro exclusivamente para Windows**.
- **Comportamiento vinculante:** la interfaz y distribución de MyCalibro serán de escritorio para Windows. No se aprueba portar MyCalibro a macOS, Linux, Android ni iOS en esta decisión.
- **Objetivo de interoperabilidad comunicado expresamente:** permitir que **BibliotecaAI, aplicación independiente para Android**, se conecte a la biblioteca de MyCalibro, que el propietario desea poder alojar en **Google Drive**. Esto **no** cambia la elección de Windows: Android utilizaría BibliotecaAI como cliente de la biblioteca compartida y no una versión móvil de MyCalibro.
- **Separación de responsabilidades:** MyCalibro administra su catálogo y archivos en Windows; BibliotecaAI tiene sus propias funcionalidades y repositorio. La interoperabilidad futura requerirá un formato de intercambio/almacenamiento estable y reglas explícitas de acceso. Las modificaciones del código Android se tratarán en su repositorio, no como cambios del código Windows.
- **Compatibilidad con PROD-002:** el modelo híbrido debe admitir que una biblioteca administrada pueda ubicarse en una carpeta sincronizada con Drive o que existan enlaces externos, sin suponer que rutas absolutas de Windows sean utilizables en Android.
- **Compatibilidad con PROD-003:** la importación puntual **desde Calibre** sigue sin sincronización posterior. El acceso compartido entre MyCalibro y BibliotecaAI es una integración distinta; no reabre la sincronización con Calibre.
- **Restricción del sistema Android ya establecida por el propietario en el proyecto BibliotecaAI:** acceso a ubicaciones elegidas por el usuario mediante Android Storage Access Framework (SAF), con Google Drive como posible proveedor, **sin OAuth, registro ni inicio de sesión Gmail incorporados en BibliotecaAI**. Verificar esta compatibilidad al diseñar y probar la integración en el repositorio Android.
- **No confundir biblioteca compartida con backup:** el respaldo ZIP de BibliotecaAI, con sustitución del mismo archivo, es un procedimiento de copia/restauración, **no** por sí mismo un catálogo vivo sincronizado con MyCalibro.
- **Pendiente de concreción:** topología de la carpeta de Drive, funcionamiento sin conexión, política de publicación, estructura e identificadores de libros, caché, conflictos, niveles de edición y compatibilidad con SAF. No se aprueba todavía sincronización bidireccional.
- **Estado de implementación:** decisión documental. Actualmente MyCalibro no gestiona una biblioteca compartida en Drive y la conexión entre ambas aplicaciones no se ha verificado.

## PROD-008 — Nivel de acceso de BibliotecaAI a la biblioteca en Google Drive
- **Fecha de apertura:** 2026-11-23.
- **Estado:** PENDIENTE DE ELECCIÓN.
- **Contexto aprobado:** MyCalibro se ejecutará solo en Windows; BibliotecaAI Android debe poder conectarse a una biblioteca de MyCalibro ubicada en Drive. **No** usar la exportación de un ZIP de backup como sustituto de una biblioteca consultable de forma ordinaria.

| Opción | Qué haría BibliotecaAI en la biblioteca compartida | Riesgo / complejidad |
| --- | --- | --- |
| A. Consulta y descarga | Visualizar catálogo y descargar/abrir libros de Drive, sin modificar el catálogo compartido; anotaciones y progreso propios permanecen locales en Android | Más simple y resistente a conflictos; no devuelve cambios a MyCalibro |
| B. Consulta + progreso de lectura | Todo A, más intercambio controlado del progreso y datos de lectura acordados entre ambos clientes, sin editar o borrar el catálogo maestro desde Android | Requiere identidad estable de libros, formatos compartidos, resolución de cambios concurrentes y pruebas sin conexión |
| C. Biblioteca editable desde ambos | Consultar, incorporar y modificar libros y metadatos desde ambas aplicaciones y reconciliar cambios sobre Drive | Máximo alcance, pero alto riesgo de conflictos, duplicados, borrados y corrupción del catálogo compartido |

**Recomendación técnica, no aprobada:** empezar con A como primera entrega de una conexión real y decidir si alcanzar B después. La elección de B o C deberá incluir una estrategia comprobable de reconciliación, versionado/atomicidad, recuperación y funcionamiento offline. En todos los casos, aprovechar el acceso SAF ya fijado para BibliotecaAI y no introducir OAuth propio como requisito.

**Cuestiones técnicas no aprobadas:** carpeta de Drive como ubicación gestionada por sincronización de escritorio, formato de catálogo portable (sin rutas absolutas dependientes de Windows), funcionamiento del lector al quedar sin conexión, selección manual de carpeta y permisos persistentes SAF, y relación con los respaldos ZIP existentes. Estas decisiones no deben inferirse de la mera elección de Drive.

## Pendientes de debates posteriores
- Detalle de importación puntual desde Calibre: campos, duplicados, política de copia/vínculo y recuperación (PROD-003, aprobada).
- Detalles de implementación del lector básico EPUB/PDF, navegación, visualización y última posición (PROD-004, aprobada).
- IA para organización y análisis de cada libro (PROD-005, aprobada); consentimiento revocable por libro para envío de contenido (PROD-006, aprobada).
- MyCalibro exclusivamente Windows y futura conexión a BibliotecaAI Android vía Drive (PROD-007, aprobada); nivel de acceso compartido pendiente (PROD-008, abierta); instalación y actualizaciones.
- Copia y restauración, compatibilidad Drive/SAF, sincronización entre productos según PROD-008 y dispositivos.

## Historial de decisiones
| ID | Fecha | Estado | Resultado |
| --- | --- | --- | --- |
| PROD-001 | 2026-11-23 | Aprobada | Opción C: plataforma integral de biblioteca, lectura e IA |
| PROD-002 | 2026-11-23 | Aprobada | Opción C: almacenamiento híbrido (archivos administrados y vinculados) |
| PROD-003 | 2026-11-23 | Aprobada | Opción A: importación puntual sin sincronización posterior con Calibre |
| PROD-004 | 2026-11-23 | Aprobada | Opción A: lector básico EPUB/PDF, navegación, visualización y posición |
| PROD-005 | 2026-11-23 | Aprobada | Opción B: IA para organización y análisis por libro bajo demanda |
| PROD-006 | 2026-11-23 | Aprobada | Opción B: consentimiento por libro, revocable, antes de enviar texto a IA externa |
| PROD-007 | 2026-11-23 | Aprobada | Opción A: MyCalibro solo Windows; BibliotecaAI Android como posible cliente de biblioteca compartida en Drive |
| PROD-008 | 2026-11-23 | Pendiente | Nivel de acceso de BibliotecaAI a la biblioteca MyCalibro alojada en Drive |
