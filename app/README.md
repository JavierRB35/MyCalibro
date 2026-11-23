# Mis libros

Una biblioteca de escritorio sencilla hecha con Rust, eframe y egui. Permite buscar libros, filtrarlos por tema o etiqueta, añadir y editar fichas y guardar la colección localmente.

## Ejecutar

Desde esta carpeta, ejecuta `cargo run` para abrir la aplicación. `cargo build` compila el programa. Los cambios se guardan en `library.json` dentro de la carpeta de biblioteca elegida; inicialmente `%LOCALAPPDATA%\MyCalibro` en Windows. Requiere Rust estable >= 1.89 (las dependencias bloqueadas pueden requerir una versión posterior).

Para guardar una captura sin activar la ventana, ejecuta `cargo run -- --screenshot shots/now.png`. La ventana se mantiene detrás de las demás, espera a dibujar y guarda la imagen PNG.

Las bibliotecas nuevas comienzan vacías. El fixture histórico del repositorio no se carga automáticamente. Introduce nuevos libros soltándolos dentro de la lista de la biblioteca: no hay un botón separado para abrir archivos. Tras incorporarlo, la app prueba Gemini primero y pasa automáticamente a Groq si Gemini falla o devuelve una respuesta inválida. Arrastra un ebook a la ventana; se aceptan EPUB, MOBI, AZW3, PDF, TXT, HTML, RTF, FB2, DOCX y otros formatos comunes. Cuando Calibre está instalado, se leen los metadatos del archivo. Configura Gemini o Groq en «Configurar IA» para clasificar y etiquetar automáticamente los libros importados, o vuelve a clasificar una ficha existente desde sus detalles. La IA recibe el título, autor, descripción y etiquetas existentes, no el archivo completo. Si guardas una clave después de importar un libro, la app clasifica automáticamente el libro seleccionado; también puedes pedir otra clasificación desde sus detalles. Las claves se guardan en el almacén seguro de credenciales de Windows; la conexión requiere internet y una clave válida. En Windows, al conectar por USB un Kindle, Kobo o NOOK que aparezca como unidad, la app detecta su biblioteca Documents y el formato recomendado (AZW3 para Kindle, EPUB para Kobo/NOOK). Arrastra un libro de la lista hasta la tarjeta del lector para copiarlo; si hace falta otro formato, Calibre lo convierte antes. Lectores conectados solo por MTP y no como unidad no se detectan en esta versión. Sin clave, se conserva la clasificación local demostrativa por palabras clave.

## Catálogo, ubicación y recuperación

«Abrir carpeta de biblioteca» abre una carpeta existente y recuerda la selección en el directorio de datos del usuario. No traslada libros ni recrea una carpeta externa que haya desaparecido. Los vínculos antiguos conservan sus rutas; mover una biblioteca no mueve sus archivos externos.

El guardado utiliza un temporal en el mismo volumen, sincronización a disco y reemplazo atómico. Antes de sustituir un catálogo válido se conserva su versión anterior en `library.previous.json`. Un bloqueo de sistema evita escritores simultáneos de MyCalibro en esta máquina. Si el catálogo cambia externamente desde su lectura, se rechaza la escritura. La sincronización de Drive entre equipos no participa en ese bloqueo: no se debe editar simultáneamente desde varios equipos.

Un JSON ilegible, un error de lectura o un bloqueo impiden editar y guardar. Los errores de guardado permanecen visibles; permiten reintentar o exportar los cambios pendientes a otra carpeta vacía. El cierre avisa cuando hay cambios sin guardar. No se cargan ejemplos en sustitución de datos dañados.

Para recuperar una versión anterior o importar el antiguo `app/library.json`, use «Recuperar/importar catálogo antiguo», elija el JSON y una carpeta vacía. Se valida antes de escribir y se preserva el origen. Esta operación recupera el catálogo y sus referencias; **no es la copia completa de libros prevista en PROD-008**.

## Envío a lectores

Las copias compatibles conservan extensión y bytes (por ejemplo, PDF sigue siendo `.pdf` en Kindle). Las conversiones requieren Calibre y se preparan en un directorio temporal. Se comprueban salida no vacía, firma del formato y, para EPUB, contenedor ZIP con `mimetype` y `META-INF/container.xml`. Las copias se comparan byte a byte antes de publicarlas. La publicación no sobrescribe nombres existentes, incluso ante colisiones concurrentes.

Estas verificaciones detectan errores de formato y copia; no certifican renderizado completo, DRM ni compatibilidad del firmware. No se afirma validación física USB/MTP sin pruebas reales del dispositivo.


## Almacenamiento híbrido y respaldo completo

En «Biblioteca y copias» se configura la importación vinculada (predeterminada) o administrada. La administrada copia sin mover el original y usa rutas relativas a la biblioteca. Puede conservar nombres/estructura del origen o crear carpetas por autor/obra; los cambios de política afectan a nuevas importaciones. El backend acepta la carpeta base para conservar subcarpetas en lotes. Las coincidencias de nombre nunca sustituyen contenido diferente. El catálogo antiguo sigue siendo compatible mediante `file_path`; los formatos nuevos se guardan en `files`, con tipo de almacenamiento y SHA-256.

«Crear copia» genera una carpeta independiente, fuera de la biblioteca, con catálogo, archivos administrados, preferencias y manifiesto SHA-256. Se puede incluir los externos accesibles; cualquier exclusión queda visible y registrada. Un archivo administrado ausente o alterado aborta el respaldo. Las claves IA siguen en el almacén del sistema y no se respaldan.

«Verificar copia» comprueba manifiesto, tamaños, huellas y referencias. «Restaurar» crea siempre una subcarpeta nueva en el destino, valida antes de abrirla y conserva tanto la biblioteca actual como los originales. Los externos incluidos se recuperan bajo `external/` y sus vínculos apuntan a las copias recuperadas; los excluidos mantienen sus rutas anteriores. La distinción administrado/vinculado se conserva.

Las copias son manuales y no se eliminan automáticamente. No hay todavía programador ni política automática de retención. La biblioteca debe permanecer disponible durante la operación. El respaldo no es una sincronización entre equipos.
