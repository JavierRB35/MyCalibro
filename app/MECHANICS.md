# Mecánicas de la aplicación original

## Evidencia del código

Se inspeccionó el ejecutable Windows x86-64 calibre.exe (0x140000000 image base, 70.920 bytes, cinco funciones). Es un lanzador, no la implementación de la biblioteca.

- `0x140001000` (`FUN_140001000`): obtiene la ruta del ejecutable; exige que mida menos de `0xe6` unidades UTF-16 (230), busca el separador de ruta y anexa `app\\bin`. Carga `ucrtbase.dll` y `calibre-launcher.dll`; busca `execute_python_entrypoint`.
- `0x140001150` (`FUN_140001150`): mide cadenas UTF-16 en un límite de `0xfa` (250) unidades.
- `0x1400011ac` (`FUN_1400011ac`): muestra errores en una ventana de Windows.
- `0x140001250` (`FUN_140001250`): recoge el error del sistema y lo muestra.
- `0x1400012d0` (`entry`): invoca el punto de entrada con `calibre`, `calibre.gui_launch`, `calibre`, `1`; libera el lanzador y sale.

La función de entrada del lanzador `calibre-launcher.dll` (`0x1800050c4`) llama a `FUN_180004f9c` para los eventos de carga/descarga de la DLL. En `0x18004aa70` está el nombre del módulo `calibre/gui_launch.pyc`; el lanzador usa Python embebido. La búsqueda de cadenas en este DLL no encontró reglas de categorías y el mapeo de `calibre/gui_launch.pyc` no enlaza con una función de biblioteca nativa. Las cinco funciones de `calibre.exe` y el inicializador del DLL fueron descompilados; ninguna implementa etiquetas o clasificación temática.

La instalación también incluye `app/bin/python-lib.bypy.frozen` (86.189.472 bytes), pero no tiene cabecera CPython ni es reconocida por el extractor disponible. Buscar cadenas dentro de sus bytes no permite descomprimir ni recuperar funciones, así que no hay reglas temáticas del programa completo que puedan verificarse. La app nueva conserva una estimación independiente: tema por coincidencia insensible a mayúsculas en título, autor y descripción; si no coincide, «Sin clasificar». Las etiquetas se editan por libro y sirven para filtrar. Estas reglas y libros de muestra son ejemplos propios, no comportamiento atribuido al original.

## Conversión de formatos

Se inspeccionó y descompiló el lanzador original `C:\Program Files\Calibre2\ebook-convert.exe` (versión del producto 9.15.0; PE x86-64, cinco funciones).

- `0x140001000` (`FUN_140001000`) resuelve la carpeta del ejecutable, agrega `app\\bin`, carga `calibre-launcher.dll` y obtiene `execute_python_entrypoint`.
- `0x1400012d0` (`entry`) llama a ese punto de entrada con `("ebook-convert", "calibre.ebooks.conversion.cli", "main", 0)` y termina con el código que devuelve el programa.
- En las cadenas referenciadas por `entry` se encuentran `calibre.ebooks.conversion.cli` (`0x140002418`) y `ebook-convert` (`0x140002458`). No se encontraron reglas ni fórmulas de conversión en este pequeño ejecutable: la conversión se delega al módulo Python de Calibre.

La aplicación usa el ejecutable instalado en `C:\\Program Files\\Calibre2\\ebook-convert.exe` (o uno localizado en PATH) y le pasa la ruta de entrada y la ruta de salida como argumentos de la línea de comandos. La selección de la extensión de salida es una opción de la aplicación; no se atribuyen al lanzador interno formatos específicos.

## Importación y clasificación con IA

La app acepta libros con extensiones comunes (EPUB, MOBI, AZW/AZW3/AZW4, KFX, PDF, TXT, HTML, RTF, FB2, DOCX, DJVU, PDB y formatos de cómic/archivo listados en `SUPPORTED_BOOK_EXTENSIONS`, `app/src/main.rs`). Se descompiló también `ebook-meta.exe`: su entrada `0x1400012d0` llama a `calibre.ebooks.metadata.cli.main` con el nombre `ebook-meta` y argumento final `0`; su inicializador `0x140001000` prepara `app\\bin` y carga `calibre-launcher.dll`. El lanzador confirma que existe una interfaz de consola para metadatos, pero no contiene las reglas o el formato de salida de Python. La reconstrucción al soltar un archivo usa la salida de consola de `ebook-meta` para obtener los metadatos compatibles (título, autor, etiquetas, año y descripción), guarda la ruta y no envía el contenido completo del libro a la IA. La lista exacta de formatos y el análisis de texto de la salida pertenecen a esta reconstrucción, no se verificaron en el ejecutable.

La clasificación usa metadatos y descripción mediante Gemini (`gemini-2.5-flash`) o Groq (`llama-3.3-70b-versatile`). Gemini es siempre el primer intento. Solo si Gemini falla por red, API o respuesta inválida se consulta una vez a Groq; si tampoco está configurado o falla, se muestra el error y no se inventan etiquetas. La app solicita JSON con un tema de la lista admitida y etiquetas breves en español; valida el tema, limita las etiquetas a ocho, y persiste el resultado en la ficha. Las peticiones se ejecutan en segundo plano. Las claves se almacenan con el almacén de credenciales de Windows (`keyring`, servicio `mis-libros`, cuenta `Gemini` o `Groq`), nunca junto al catálogo. Los enlaces de configuración abren Google AI Studio o Groq Console. Sin clave, se abre la configuración y no se simula una clasificación de IA.

## Lectores USB

En Windows se revisan las letras de unidad y se reconocen Kindle por `system/version.txt`, Kobo por `.kobo`, y NOOK por `.adobe-digital-editions` o `NOOK`. El formato elegido por el programa para envío es AZW3 para Kindle y EPUB para Kobo/NOOK; es una regla heurística del producto nuevo, no un valor recuperado del firmware. El libro se convierte con `ebook-convert.exe` si no está ya en formato compatible, y se copia a Documents. La biblioteca del lector enumera archivos compatibles en Documents y subcarpetas hasta dos niveles. Dispositivos MTP que no exponen una letra de unidad no están soportados.