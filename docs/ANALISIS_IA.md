# Análisis por libro: contrato de integración

Motor en `analysis_backend.rs` y extracción local en `analysis_text.rs`. La conexión
de la ficha y su worker pertenece al ensamblaje de interfaz; publicar estos módulos
no acredita por sí solo el flujo visual ni una llamada real al proveedor.

`analyze(&AnalysisRequest, &ApiKeys, guard)` es bloqueante y debe ejecutarse fuera
del hilo de interfaz. El resultado contiene respuesta, proveedor utilizado, fuente
(`MetadataOnly` o `TextFragments`), citas verificadas y alcance del contexto.
Las claves solo viven en memoria y no forman parte del catálogo ni del resultado.

El guard recibe `(Provider, caracteres_de_contenido)` antes de **cada** envío,
incluido el respaldo tras error o respuesta inválida. Debe comprobar generación de
biblioteca/libro/cancelación y consultar el consentimiento **actual** compartido,
sin capturar un permiso clonado al crear el worker. En modo metadatos recibe cero:
no se exige autorización de contenido, pero siguen aplicando las comprobaciones
de biblioteca y cancelación. Los resultados de otra biblioteca deben descartarse.

El modo metadatos no abre el ebook. El modo contenido extrae EPUB/TXT localmente;
otros formatos requieren `ebook-convert` y un temporal local, con límite de dos
minutos. No hay OCR. Se seleccionan como máximo cuatro fragmentos y 6000 caracteres,
con un resto del texto siempre excluido. Las respuestas señalan que el contexto es
parcial. Las ubicaciones de texto convertido son párrafos, **no páginas PDF**.
Las citas deben pertenecer literalmente al fragmento indicado; referencias
inventadas o afirmaciones sin citas verificables se rechazan. La comprobación
literal no certifica que la interpretación semántica de la IA sea correcta.

Modelos del motor: `gemini-2.5-flash-lite` y `openai/gpt-oss-20b` en Groq, con JSON
validado localmente y redirecciones HTTP deshabilitadas. Consultado el 23/11/2026:
[Gemini](https://aistudio.google.com/docs/deprecations#gemini-2.5-flash-models)
restringe 2.5 Flash a usuarios anteriores y anuncia transición el 01/12/2026;
2.5 Flash-Lite continúa disponible.
[Groq](https://console.groq.com/docs/deprecations) retiró Llama 3.3 para planes
free/developer el 16/08/2026; GPT-OSS admite salida JSON.

Pruebas sin claves reales: ocho casos de extracción, minimización Unicode,
metadatos sin lectura de archivo, citas inválidas, ausencia de permiso y revocación
entre proveedor principal y respaldo. Antes de conectar los módulos a main:
`cargo test --test analysis_pipeline analysis_`. El pequeño harness temporal debe
retirarse al declararlos en main para no duplicar sus pruebas unitarias.
