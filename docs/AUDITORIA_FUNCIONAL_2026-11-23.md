# MyCalibro — auditoría funcional y agenda de evolución

**Fecha:** 2026-11-23. **Fuente:** rama main, commit de partida c4db935e871461053a5f73698da7d5d8bb77801e. **Tipo:** inspección estática de código y documentación; **sin ejecución local, sin hardware y sin llamadas reales a IA**.

## 1. Resumen ejecutivo
MyCalibro (ventana «Mis libros») es una app de escritorio Rust 2021 con eframe/egui 0.31. Versionada como 0.1.0. No es Calibre completo ni un lector integrado. Es un catálogo local que usa herramientas Calibre externas para metadatos y conversiones.
La rama main contiene un único archivo fuente principal (app/src/main.rs, 1.851 líneas), Cargo.lock, una biblioteca JSON, documentación y evidencias históricas. En la inspección no se encontraron .github/workflows, AGENTS.md previo, issues ni PR. craft.json informa de verificaciones históricas de compilación y dibujo de ventana, no de pruebas funcionales exhaustivas.

**Leyenda:** I = implementado en código; P = parcial/condicionado; A = ausente del código inspeccionado. «I» no significa validado en ejecución real.

## 2. Inventario de funcionalidades existentes

| Área | Estado | Evidencia y alcance |
| --- | --- | --- |
| Interfaz de escritorio | I | Ventana egui, panel lateral, lista y panel de detalle, tamaño inicial 1360×850, mínimo 950×620: main.rs:675-1112 y 1825-1850. |
| Añadir ficha manual | I | Título obligatorio, autor, año, descripción y etiquetas: main.rs:1114-1158. |
| Editar fichas | I | Título, autor, año, descripción y etiquetas editables en detalle: main.rs:1000-1109. |
| Eliminar libro del catálogo | I con riesgo | Borrado inmediato, sin confirmación; afecta la ficha, no se implementa eliminación del archivo: main.rs:1091-1101. |
| Buscar | I | Búsqueda insensible a mayúsculas en título, autor, descripción y etiquetas: main.rs:178-205. |
| Filtrar por tema | I | Temas prefijados: Ficción, Ciencia, Historia, Fantasía, Misterio, Tecnología y Sin clasificar: main.rs:1591-1653. |
| Filtrar por etiqueta | I | Etiquetas editables, navegación lateral, filtro por igualdad ignorando mayúsculas: main.rs:675-845. |
| Clasificación local | I | Heurística por palabras clave sobre título, autor y descripción; no es un modelo de IA: main.rs:1611-1653. |
| Importación drag-and-drop | I/P | Al soltar archivos de extensiones aceptadas, crea ficha y guarda ruta externa, con metadatos o nombre del archivo: main.rs:214-276 y 352-371. Sin selector específico de importación. |
| Tipos de archivo reconocidos | P | EPUB, PDF, MOBI, AZW*, TXT, DOCX, FB2, DJVU, cómics y otros; validados solo por extensión, no por decodificación/lectura real: main.rs:12-16 y 1225-1233. |
| Extracción de metadatos | P | Usa ebook-meta de Calibre si está instalado. Parser de texto lineal: title, author, tags, comments/description y published. Puede perder descripciones multilínea: main.rs:1235-1319. |
| Clasificación automática IA | P | Prompts con título, autor, descripción y etiquetas; Gemini gemini-2.5-flash primero y Groq llama-3.3-70b-versatile si falta clave de Gemini o falla. Sin análisis de contenido completo: main.rs:278-301 y 1321-1414. |
| Etiquetas sugeridas por IA | P | Procesa JSON, restringe tema a catálogo, toma hasta 8 etiquetas y combina sin duplicados por mayúsculas: main.rs:324-349 y 1388-1414. Dependiente de credenciales/red/API. |
| Gestión de claves IA | I/P | Configura Gemini/Groq, almacena en keyring del sistema y ofrece enlaces a proveedores: main.rs:1160-1223. No se probó en Windows. |
| Persistencia del catálogo | I con riesgo | Lee/escribe app/library.json automáticamente. No escritura atómica ni backup. Si falla lectura/parseo carga ejemplos: main.rs:120-176, 1720-1730. |
| Conversión manual | P | Diálogo con selector de archivo y salida a EPUB/MOBI/AZW3/PDF/TXT/HTML/RTF/FB2; ejecuta ebook-convert externo y deja archivo junto al origen: main.rs:1416-1576. |
| Detectar lectores USB | P | Por letras de unidad Windows: Kindle, Kobo y NOOK; MTP no soportado: main.rs:462-503. |
| Consultar ficheros del dispositivo | P | Lista archivos del directorio Documents/My Documents hasta profundidad 2, no es biblioteca de metadatos del dispositivo: main.rs:504-571. |
| Enviar libro al lector | P con defecto | Arrastrar ficha a dispositivo; copia o convierte mediante Calibre, en hilo independiente. Error de extensión descrito debajo: main.rs:307-322 y 573-673. |
| Captura de pantalla | I | Parámetro --screenshot y guardado PNG para inspección técnica: main.rs:373-409 y 1732-1746. |

## 3. Defectos y riesgos comprobables por código

### CRÍTICO — Integridad de catálogo
- main.rs:123-133: si un JSON existe pero no puede leerse o parsearse, la app carga libros ficticios, y un guardado posterior puede sustituir los datos del usuario.
- main.rs:168-176: std::fs::write escribe directamente el archivo; no hay archivo temporal, copia de recuperación ni restauración.
- main.rs:1720-1730: la ubicación del catálogo depende de subir tres directorios desde el ejecutable; es razonable en cargo target/debug, pero frágil en un empaquetado/instalación real.

### ALTO — Envío a dispositivos
- main.rs:573-607 asigna siempre extensión AZW3 a Kindle o EPUB a Kobo/NOOK.
- main.rs:628-636 permite copiar directamente PDF/TXT y, para Kindle, MOBI; por tanto puede escribir el mismo contenido con extensión distinta (p. ej. un PDF guardado como .azw3). Una extensión distinta no convierte el archivo, y el resultado puede ser ilegible.
- main.rs:474-503 detecta solo dispositivos montados como unidad; no funciona con lectores exclusivamente MTP.
- main.rs:638-669 no verifica de forma independiente que el archivo convertido o copiado resulte legible antes de informar éxito.

### MEDIO — Usabilidad y mantenimiento
- El archivo importado no se incorpora al almacenamiento administrado: se conserva ruta absoluta externa (main.rs:265). Mover/borrar el origen rompe el vínculo.
- Los duplicados se detectan solo por igualdad exacta de ruta (main.rs:223-230), no por ISBN, hash, contenido o variación de nombre.
- Eliminar una ficha no pide confirmación ni ofrece deshacer (main.rs:1091-1101).
- app/library.json está versionado en Git y contiene datos de ejemplo y una ruta absoluta de usuario; conviene separar catálogo privado de los ejemplos controlados por versiones.
- La importación con ebook-meta y la conversión del diálogo se ejecutan de manera síncrona en interfaz (main.rs:231 y 1488), con potencial bloqueo durante operaciones lentas.
- Las categorías son rígidas y heurísticas simples; no hay jerarquía ni taxonomía ampliable.
- El parser de metadatos usa líneas con dos puntos y no interpreta adecuadamente descripciones multilínea.

## 4. Funcionalidades no presentes en el código actual
No se encontraron: lectura integrada de EPUB/PDF; miniaturas de portadas; visualización por cuadrícula; marcador/progreso; anotaciones o subrayados; biblioteca física separada; ISBN, editorial, idioma, serie, número de páginas o valoración; colecciones/estanterías; formatos múltiples por una misma obra; importación masiva de carpetas; organización y copia de ficheros a almacenamiento propio; detección y fusión robusta de duplicados; edición masiva; metadatos/portadas online; sincronización con catálogo Calibre (metadata.db); búsqueda dentro del contenido; biblioteca de préstamos; copias y restauración; exportación de catálogo; recuperación tras fallo; paquete instalable y autoactualización; pruebas E2E de dispositivos; CI; autenticación o nube. Esta lista describe ausencias actuales, no funcionalidades acordadas para desarrollar.

## 5. Estado de pruebas
- En main.rs:1748-1823 existen **7 pruebas unitarias** sobre reglas básicas, extensiones, formato, selección de lector y ausencia de claves; ninguna cubre transferencia real, restauración, importación completa ni llamadas IA.
- craft.json conserva verificaciones históricas de compilación y captura de ventana, y reconoce que las mecánicas no se comprobaron contra el comportamiento runtime.
- En esta auditoría no se ejecutaron cargo check, cargo test, la ventana, Calibre, APIs externas ni dispositivos. No declarar esos escenarios validados.
- No se observó configuración de GitHub Actions.

## 6. Prioridades técnicas para la próxima fase
1. Proteger el catálogo: ruta robusta, escritura atómica, backups verificables, recuperación segura y separación de datos privados del repositorio.
2. Corregir el envío a lectores: preservar extensión real al copiar formatos compatibles; convertir solo cuando haga falta; validar resultado y errores.
3. Mejorar importación: selector, carpetas, progreso, no bloquear interfaz, integridad, duplicados y ubicación administrada de libros.
4. Ampliar metadatos, portadas, colecciones, filtros, operaciones masivas e interoperabilidad Calibre.
5. Incorporar el lector básico EPUB/PDF aprobado (PROD-004), con navegación, preferencias de visualización y última posición; no presupone notas, subrayados ni OCR.
6. Preparar empaquetado, pruebas reales y mantenimiento continuo.

## 7. Dirección de producto y debate vigente
**PROD-001 APROBADA (2026-11-23):** MyCalibro evolucionará a una **plataforma integral de biblioteca, lectura e inteligencia artificial (opción C)**. Incluye como horizonte funcional gestión avanzada de libros y dispositivos, lectura integrada y funciones de IA; el alcance concreto del lector está delimitado por PROD-004. Esta decisión es de producto, **no equivale a afirmar que esas funciones estén implementadas ni autoriza cambios de código sin especificar su alcance**. El inventario de los apartados anteriores mantiene su valor como estado actual.

**Registro canónico de decisiones:** [docs/DECISIONES_PRODUCTO.md](DECISIONES_PRODUCTO.md). Las futuras decisiones aprobadas se registrarán allí y deberán respetar el protocolo recogido en AGENTS.md.

**PROD-002 APROBADA (2026-11-23):** sistema híbrido de almacenamiento. MyCalibro deberá admitir libros administrados y libros referenciados externamente, distinguiendo ambos casos y preservando archivos y datos existentes. **Todavía no está implementado**: el inventario anterior describe el estado real del código.

**PROD-003 APROBADA (2026-11-23):** importación puntual desde Calibre (opción A). Los libros y metadatos se importarán sin escrituras a la biblioteca de origen y **sin sincronización posterior**; el catálogo de cada aplicación evolucionará de forma independiente. El modo de almacenamiento administrado/vinculado sigue lo dispuesto en PROD-002. **Aún no implementado**: no confundir con la importación actual de archivos individuales.

**PROD-004 APROBADA (2026-11-23):** lector básico EPUB/PDF (opción A), con navegación, preferencias de visualización y recuperación de posición. No incluye subrayados, notas, OCR ni IA contextual dentro del lector. **Aún no implementado**.

**PROD-005 APROBADA (2026-11-23):** IA para organización del catálogo y análisis de cada libro bajo demanda (opción B). Incluye enriquecer fichas y etiquetas y, cuando se disponga de texto y autorización, generar argumentos/resúmenes, análisis y respuestas fundamentadas en el contenido. No habilita consultas transversales de toda la colección ni IA en el lector básico. La clasificación por metadatos existente no acredita el funcionamiento de estas futuras capacidades; **las nuevas funciones aún no están implementadas**.

**PROD-006 APROBADA (2026-11-23):** consentimiento expreso por libro y revocable (opción B) antes de transmitir su contenido a proveedores externos de IA. Debe informarse el proveedor y los datos enviados, minimizar los fragmentos transmitidos y respetar la revocación para futuras consultas. El comportamiento de clasificación por metadatos ya existente es distinto. **Esta política todavía no está implementada.**

**PROD-007 APROBADA Y ACLARADA (2026-11-23):** MyCalibro será exclusivamente Windows (opción A). Se desea que su biblioteca pueda residir en una carpeta elegida por el usuario, incluida una sincronizada con Google Drive. **No** se requiere ninguna integración entre aplicaciones, API de Drive ni sistema de conexión remoto; la idea anterior de interoperabilidad quedó expresamente descartada. Las carpetas sincronizadas presentan riesgos de concurrencia y no equivalen a una copia de seguridad. La selección y traslado seguro de biblioteca aún requieren implementación.

**PROD-008 APROBADA (2026-11-23):** copia de seguridad **completa y configurable** (opción C), con catálogo, metadatos y configuración, libros administrados y opción de incluir libros externos vinculados. Deberá ofrecer manifiesto, verificación de integridad y restauración segura, informando de exclusiones, errores y referencias inaccesibles. El formato, automatización, retención y destino concreto siguen sin decidirse. **Aún no implementada.**

**PROD-009 APROBADA (2026-11-23):** organización física configurable (opción C) para libros administrados: conservar nombres y estructura de origen o utilizar carpetas por autor/obra, con colecciones y etiquetas independientes de las rutas físicas. Los archivos vinculados externamente no se moverán ni renombrarán de forma automática. Los cambios de ubicación deben preservar integridad, vínculos y respaldos. **Aún no implementada.**

**PROD-010 APROBADA (2026-11-23):** asistente de importación masiva (opción B), con lectura recursiva de archivos y carpetas, progreso y vista previa; detectar coincidencias por datos e identificadores disponibles y exigir revisión del usuario antes de fusionar registros, omitir entradas o asociar formatos a un libro existente. No confundir coincidencias de título/autor con identidad de edición. **Aún no implementada.**

**PROD-011 — CRITERIO APROBADO (2026-11-23):** utilizar **por defecto los metadatos que vengan en el propio libro**. No sobrescribirlos automáticamente con búsqueda externa o inferencia de IA ni introducir descarga automática de portadas. Mantener edición manual y distinguir datos derivados de información bibliográfica original. Verificar la extracción real por formato; **no se ha implementado ninguna capacidad adicional por esta decisión**.

**Transición al desarrollo:** el propietario ha indicado que no desea seguir debatiendo funcionalidades salvo cuestiones de verdadera criticidad y facilitará la composición y organización del equipo. El siguiente paso es preparar plan de ejecución y reparto de tareas, con prioridad inicial a los riesgos de integridad y persistencia ya detectados.

**Detalles aplazados no bloqueantes:** organización lógica y colecciones, frecuencia/formato de backups, instalación y actualizaciones. Resolver durante ejecución solo cuando una decisión crítica lo exija.

## 8. Continuidad
Toda propuesta futura se comparará con este inventario. Una decisión aceptada actualizará la documentación; un cambio de código exigirá pruebas proporcionales al riesgo y evidencia verificable. Mantener main como rama normal y evitar ramas salvo cambios críticos. No tocar archivos reales ni datos del usuario sin una estrategia de preservación.
