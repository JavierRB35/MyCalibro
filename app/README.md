# Mis libros

Una biblioteca de escritorio sencilla hecha con Rust, eframe y egui. Permite buscar libros, filtrarlos por tema o etiqueta, añadir y editar fichas y guardar la colección localmente.

## Ejecutar

Desde esta carpeta, ejecuta `cargo run` para abrir la aplicación. `cargo build` compila el programa. Los cambios se guardan en `library.json` dentro de la carpeta de biblioteca elegida; inicialmente `%LOCALAPPDATA%\MyCalibro` en Windows. Requiere Rust estable >= 1.89 (las dependencias bloqueadas pueden requerir una versión posterior).

Para guardar una captura sin activar la ventana, ejecuta `cargo run -- --screenshot shots/now.png`. La ventana se mantiene detrás de las demás, espera a dibujar y guarda la imagen PNG.

Las bibliotecas nuevas comienzan vacías. El fixture histórico del repositorio no se carga automáticamente. «Importar libros…» permite seleccionar archivos o carpetas, también mediante arrastre, y revisar el lote antes de importar. Se aceptan EPUB, MOBI, AZW3, PDF, TXT, HTML, RTF, FB2, DOCX y otros formatos comunes. El asistente permite importar, omitir o asociar formatos sin sustituir los metadatos de la ficha destino. La preparación ocurre en segundo plano; la cancelación conserva los archivos ya completados y el informe distingue errores y archivos no procesados.

Los EPUB se leen directamente: título, autores, fecha, descripción, etiquetas, identificadores, otros campos Dublin Core y portada del OPF. Se conserva la procedencia en `bibliography`. Para los restantes formatos se utiliza `ebook-meta` de Calibre cuando está instalado; si no se pueden leer metadatos o falta título, se informa y se usa el nombre del archivo como presentación. No se descargan metadatos ni portadas de terceros. Las portadas extraídas se copian a `covers/` y se verifican en los respaldos.

«Desde Calibre…» importa una biblioteca puntualmente a partir de una copia aislada de `metadata.db`. Calibre debe estar cerrado, sin transacciones pendientes. Se conservan los campos bibliográficos soportados, identificadores y portada; los formatos de un mismo registro se agrupan en una ficha. No se modifica la biblioteca de origen ni se sincronizan cambios posteriores. Los registros sin formatos accesibles se avisan y no generan una ficha automática.

La clasificación por IA puede solicitarse desde los detalles de una ficha. Gemini tiene prioridad y Groq actúa como alternativa si hay clave disponible. Se envían título, autor, descripción y etiquetas, no el ebook completo. Las claves se guardan en el almacén de credenciales de Windows. Esta clasificación de metadatos no equivale a analizar el contenido. Sin clave se conserva la clasificación local por palabras clave.

En Windows, un Kindle, Kobo o NOOK que aparezca como unidad puede recibir archivos arrastrados desde la biblioteca. Las copias mantienen su formato; las conversiones necesarias requieren Calibre. MTP no está implementado.

## Catálogo, ubicación y recuperación

«Abrir carpeta de biblioteca» abre una carpeta existente y recuerda la selección en el directorio de datos del usuario. No traslada libros ni recrea una carpeta externa que haya desaparecido. Los vínculos antiguos conservan sus rutas; mover una biblioteca no mueve sus archivos externos.

El guardado utiliza un temporal en el mismo volumen, sincronización a disco y reemplazo atómico. Antes de sustituir un catálogo válido se conserva su versión anterior en `library.previous.json`. Un bloqueo de sistema evita escritores simultáneos de MyCalibro en esta máquina. Si el catálogo cambia externamente desde su lectura, se rechaza la escritura. La sincronización de Drive entre equipos no participa en ese bloqueo: no se debe editar simultáneamente desde varios equipos.

Un JSON ilegible, un error de lectura o un bloqueo impiden editar y guardar. Los errores de guardado permanecen visibles; permiten reintentar o exportar los cambios pendientes a otra carpeta vacía. El cierre avisa cuando hay cambios sin guardar. No se cargan ejemplos en sustitución de datos dañados.

Para recuperar una versión anterior o importar el antiguo `app/library.json`, use «Recuperar/importar catálogo antiguo», elija el JSON y una carpeta vacía. Se valida antes de escribir y se preserva el origen. Esta operación copia y verifica también los archivos administrados y las portadas referenciadas, tomando como origen la carpeta donde está el JSON. Los vínculos externos mantienen sus rutas. El catálogo nuevo se publica solo cuando terminan las copias; los originales se conservan. La exportación de cambios pendientes utiliza el mismo procedimiento y ambas operaciones se ejecutan en segundo plano. Si falla, pueden quedar copias parciales en el destino, sin catálogo publicado; para reintentar elija otra carpeta vacía. No recupera preferencias ni estado auxiliar: para eso utilice el respaldo completo de PROD-008.

## Envío a lectores

Las copias compatibles conservan extensión y bytes (por ejemplo, PDF sigue siendo `.pdf` en Kindle). Las conversiones requieren Calibre y se preparan en un directorio temporal. Se comprueban salida no vacía, firma del formato y, para EPUB, contenedor ZIP con `mimetype` y `META-INF/container.xml`. Las copias se comparan byte a byte antes de publicarlas. La publicación no sobrescribe nombres existentes, incluso ante colisiones concurrentes.

Estas verificaciones detectan errores de formato y copia; no certifican renderizado completo, DRM ni compatibilidad del firmware. No se afirma validación física USB/MTP sin pruebas reales del dispositivo.


## Almacenamiento híbrido y respaldo completo

En «Biblioteca y copias» se configura la importación vinculada (predeterminada) o administrada. La administrada copia sin mover el original y usa rutas relativas a la biblioteca. Puede conservar nombres/estructura del origen o crear carpetas por autor/obra; los cambios de política afectan a nuevas importaciones. El backend acepta la carpeta base para conservar subcarpetas en lotes. Las coincidencias de nombre nunca sustituyen contenido diferente. El catálogo antiguo sigue siendo compatible mediante `file_path`; los formatos nuevos se guardan en `files`, con tipo de almacenamiento y SHA-256.

«Crear copia» genera una carpeta independiente, fuera de la biblioteca, con catálogo, archivos administrados, preferencias y manifiesto SHA-256. Se puede incluir los externos accesibles; cualquier exclusión queda visible y registrada. Un archivo administrado ausente o alterado aborta el respaldo. Las claves IA siguen en el almacén del sistema y no se respaldan.

«Verificar copia» comprueba manifiesto, tamaños, huellas y referencias. «Restaurar» crea siempre una subcarpeta nueva en el destino, valida antes de abrirla y conserva tanto la biblioteca actual como los originales. Los externos incluidos se recuperan bajo `external/` y sus vínculos apuntan a las copias recuperadas; los excluidos mantienen sus rutas anteriores. La distinción administrado/vinculado se conserva.

Las copias son manuales y no se eliminan automáticamente. No hay todavía programador ni política automática de retención. La biblioteca debe permanecer disponible durante la operación. El respaldo no es una sincronización entre equipos.


El respaldo copia solo formatos administrados referenciados y recursos propios: `library-settings.json`, `next-book-id.json`, `library.previous.json`, `reader.json` y directorios `state/`, `covers/`, `resources/`. Los archivos ajenos de la carpeta seleccionada no se incluyen. Los vínculos siguen sujetos a la opción de externos aunque residan bajo esa carpeta. El estado auxiliar nuevo debe guardarse en `state/` o registrarse explícitamente en el respaldo.

Los detalles de ficha identifican cada archivo como administrado o vinculado y avisan si no está accesible. Si las preferencias son ilegibles, su restablecimiento conserva antes una copia de los bytes originales bajo `state/`.
