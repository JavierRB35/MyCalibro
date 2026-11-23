# Continuidad de MyCalibro — trabajo solo en GitHub

Fecha: 23 de noviembre de 2026. Por instrucción expresa del propietario, ambos agentes trabajan desde ahora únicamente sobre GitHub. No usar el clon, terminal ni escritorio local para continuar. Las pruebas anteriores conservan su evidencia; las siguientes deben ejecutarse en GitHub Actions. No declarar la aplicación terminada todavía.

## Coordinación y reservas

Canal oficial: [issue #1](https://github.com/JavierRB35/MyCalibro/issues/1). A conserva main.rs, backend, almacenamiento, importación, integración y CI. B tiene reserva exclusiva de reader_view.rs para cargar EPUB en un worker y descartar resultados obsoletos; mantener firma open_path, font_scale/theme y resto de API. Leer los últimos comentarios antes de editar. No cambiar archivos reservados por el otro agente.

## Entregado

- #5: metadatos/portada EPUB nativos, campos e identificadores conservados. #6: recuperar catálogo con recursos administrados, contador y vínculos externos sin romper rutas; exportación asíncrona.
- #7, revisión B favorable sobre 3cd574f: panel de resumen/análisis/pregunta; metadatos por defecto, consentimiento vivo Arc/RwLock antes de cada envío/fallback; cancelación por cierre/cambio/borrado. Citas literales verificadas, insuficiencia también en metadatos. Máximo 6000 caracteres y mitad del texto legible por consulta. Sin claves en catálogo ni llamadas pagadas en pruebas.
- Persistencia auxiliar detecta cambios externos, conserva corruptos, informa reintentos y bloquea cambio de biblioteca con pendientes. Backup incluye posiciones y no revive consentimientos al restaurar.
- #8: reserva MTA de vida de proceso para fábricas PDF, conserva RoInitialize/RoUninitialize por worker; prueba aislada de teardown y trabajadores sucesivos.
- #9 fusionada en 68a3eaa: XML real EPUB, URI seguras, límites, capítulos ausentes explícitos y entidades truncadas sin panic. Páginas solo visuales conservan posición; su aviso está en título de presentación, nunca en párrafos enviados a IA.
- 0fc7fb4: duplicados por SHA-256, identificadores de EPUB/Calibre y título/autor incrustados, con certeza explícita y revisión humana; ninguna fusión automática.
- 9aeba8c: preferencias de fuente/tema EPUB y zoom/tema PDF persistidas en reading-positions.json, compatibles con archivos anteriores.
- Paneles egui reubicados y desplazables; confirmación de importación visible. Capturas sintéticas en docs/evidence.

## Evidencia válida y límites

- Windows [run 45179902539](https://github.com/JavierRB35/MyCalibro/actions/runs/45179902539), source 5ff9f3c4145f2b44ad7479c0fdcb25be639b3e4e: PDF solo en proceso aislado 1/1, suite 98/98; release, instalador, apertura instalada y desinstalación conservando datos correctos. [Artefacto 14821032647](https://github.com/JavierRB35/MyCalibro/actions/runs/45179902539/artifacts/14821032647). Este código precede a la integración final de análisis/importación/lector; no certifica main actual.
- Antes de pasar a modo solo GitHub: 10/10 análisis (transporte simulado), 1/1 estado auxiliar, 10/10 backup, 38/38 importación, 5/5 posiciones/preferencias, 11/11 parser EPUB d06dc55. d1f2268 añade únicamente rustfmt al parser probado.
- GUI aislada: importar TXT, confirmar catálogo guardado/original intacto; panel de análisis, error sin claves, aviso previo, autorización con proveedores exactos, reinicio, revocación persistida. No se probaron proveedores con claves reales ni hardware USB real.
- app/library.json permanece fixture histórico sin modificaciones.

## Cómo verificar sin terminal

El workflow windows.yml mantiene workflow_dispatch y tags v*. Además un cambio explícito de ci/windows-candidate.txt en main dispara la validación Windows completa. La edición de windows.yml también la dispara. Actualizar ese marcador solo para un candidato nuevo o una configuración de prueba distinta; no repetir baterías sin motivo. Consultar runs/jobs/logs/artifacts por herramientas GitHub. No etiquetar release final antes de comprobar el candidato integrado.

## Trabajo pendiente antes de cerrar

1. Revisar/integrar PR de B para reader_view.rs asíncrono, ejecutar pruebas en Actions y validar posiciones en el lector integrado.
2. Completar o documentar con evidencia los puntos aún no cubiertos de PROD-001..011: reorganización segura/previsualizada de libros administrados existentes (las políticas actuales se aplican a importaciones nuevas); distinguir etiquetas derivadas por IA de las bibliográficas originales; mostrar portadas/metadatos preservados en la ficha. No ampliar el alcance con funciones no aprobadas.
3. Revisar casos de cancelación/errores y uso de múltiples formatos; comprobar que el lector y análisis eligen una fuente compatible, que errores de estado no se pierden y que el cambio de biblioteca invalida tareas.
4. Candidato Windows de main integrado: pruebas, instalador y flujos relevantes. La salida satisfactoria del test PDF aislado no reproduce por sí misma el equipo previo de B; mantener el alcance exacto de la evidencia.
5. Actualizar README, entrega Windows y estado de decisiones; publicar artefacto/release verificable con SHA256. No confundir objetivos documentados ni pruebas de una versión anterior con funciones finales comprobadas.

Los límites actuales incluyen lector EPUB de texto sin imágenes renderizadas, extracción de PDF para IA dependiente de Calibre (sin OCR), dispositivos USB montados como unidad (sin MTP) y ausencia de llamadas reales a proveedores durante validación. No inventar resultados para cubrirlos.

## Instrucciones de inicio para el sucesor como Agente A

El propietario ha pedido detener este chat después de publicar el relevo y reiniciar A en otro chat. No se está declarando el proyecto acabado. Todo el código de A está publicado; no hay parches locales que recuperar.

1. Asume la misión de Agente A en JavierRB35/MyCalibro, **solo GitHub**. Lee AGENTS.md, README.md, docs/DECISIONES_PRODUCTO.md, este archivo y los comentarios recientes de la issue #1. Sigue resolviendo decisiones técnicas ordinarias sin pedir permiso al propietario, dentro del alcance ya aprobado. No reabras debates de producto.
2. Preserva la reserva de B sobre reader_view.rs. Su nueva entrega es [PR #10](https://github.com/JavierRB35/MyCalibro/pull/10), head abreviado 6273285 al cierre de este chat: carga EPUB asíncrona y pruebas de cierre/cambio/error. **A todavía no la ha revisado ni probado**. Comprueba su head actual, revisión y resultados; no heredes como aprobación la aprobación de #9. #5, #6, #7, #8 y #9 sí están fusionadas.
3. La modificación de CI c632a297b104822de1d76b05dc57040fdce0621b habilitó el disparo desde GitHub mediante cambio de windows.yml o ci/windows-candidate.txt en main. Debió disparar el candidato de main vigente en ese commit; consulta Actions para identificar run y resultado. No consta resultado de ese candidato en el relevo. El conector fetch_commit_workflow_runs filtra eventos pull_request: una lista vacía no demuestra que no haya run de push. El marcador no ejecuta una rama de PR: para probar #10 antes de fusionar, usa workflow_dispatch desde GitHub o un trigger pull_request explícito y coordinado, sin recurrir a terminal local ni fingir ejecución.
4. Antes de cualquier nueva escritura, recupera archivo/SHA o head actual. Usa update_file con SHA y merge_pull_request con expected_head_sha. Las herramientas GitHub no comparten siempre el nombre del argumento de repositorio: fetch_file/update_file/merge_pull_request usan repository_full_name; issue comments y algunas herramientas de CI usan repo_full_name. ALL_TOOLS contiene sus declaraciones completas. No sobreescribas cambios concurrentes ni uses force push.
5. Avanza sobre los pendientes de alcance enumerados arriba, con commits pequeños y evidencia proporcional. Confirma especialmente separación de etiquetas IA/originales, reorganización administrada segura, visibilidad de portada y lector integrado. La UI anterior conserva los originales; cualquier reorganización debe previsualizar, copiar/verificar antes de publicar rutas y dejar recuperable un fallo de catálogo.
6. Mantén el registro de entrega exacto: source SHA, run, cantidad de pruebas, artefacto, hash del instalador y límites. El artefacto Windows 14821032647 está probado, pero es anterior a main integrado. No publiques como final un ejecutable viejo ni declares probado el hardware o las APIs reales que no se ejercieron.
7. Comunica avances en español. La coordinación entre agentes se mantiene en la issue #1; no necesita la intervención ordinaria del propietario. Al finalizar, entrega una versión instalable verificada y deja GitHub con documentación coherente, o registra de forma concreta cualquier bloqueo real.
