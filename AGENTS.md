# AGENTS.md — MyCalibro

## Fuente de verdad y alcance
- GitHub, repositorio JavierRB35/MyCalibro, es la única fuente de verdad del proyecto.
- Rama de trabajo habitual: main. No crear ramas salvo modificación crítica que haga necesario aislar y revisar el cambio.
- No modificar otro repositorio ni reutilizar supuestos de otras aplicaciones sin verificar aquí su aplicabilidad.
- El producto actual es una aplicación de escritorio Rust/eframe/egui; no confundirla con Calibre original.
- Esta guía adapta el «Protocolo de desarrollo asistido por inteligencia artificial» v1.0 facilitado por el propietario.
- Interpretar cada solicitud según su alcance: información, diagnóstico, planificación, implementación, verificación o publicación.
- Un diagnóstico no autoriza por sí solo cambios funcionales.

## Estado técnico conocido
- Entrada de aplicación: app/src/main.rs (código monolítico actual).
- Dependencias y versión: app/Cargo.toml y app/Cargo.lock.
- Datos de catálogo: app/library.json; contiene datos de ejemplo y una ruta de archivo local.
- Documento de funcionamiento reconstruido: app/MECHANICS.md.
- Estado y evidencias de la auditoría: docs/AUDITORIA_FUNCIONAL_2026-11-23.md.
- La aplicación depende de las utilidades externas ebook-meta y ebook-convert de Calibre para ciertas operaciones.
- El comportamiento con lectores USB está orientado a Windows; MTP no está implementado.
- No tratar logs o registros históricos como prueba actual de un comportamiento.

## Antes de modificar
- Inspeccionar primero la ruta afectada, sus consumidores y la rama/cabeza actuales.
- Comprobar requisitos, comportamiento que debe permanecer, impacto sobre datos y riesgo de regresión.
- No atribuir funcionalidades al programa por aparecer en el README; contrastarlas con el código.
- Localizar instrucciones existentes y evitar documentación duplicada.
- Conservar cambios de terceros; nunca hacer force push, reset destructivo o borrados masivos.
- Evitar cambios colaterales y refactorizaciones preventivas sin necesidad.
- No añadir una dependencia cuando lo resuelvan las existentes o la plataforma.

## Integridad y seguridad
- Prioridad: integridad de datos, corrección, requisitos, compatibilidad, mantenimiento, UX, rendimiento y coste.
- No sobrescribir ni eliminar archivos reales de libros durante pruebas.
- No modificar ni publicar información personal almacenada en datos de ejemplo o rutas de archivo.
- Nunca introducir claves de Gemini/Groq en código, catálogos, commits, logs ni documentación.
- Al cambiar persistencia, definir compatibilidad, escrituras seguras, recuperación, duplicados y fallo por interrupción.
- Antes de afirmar que existe backup funcional, probar restauración y recuperación de datos.
- Antes de modificar el envío a lectores, comprobar el formato real del archivo y la preservación del original.
- No asumir que una extensión renombrada equivale a una conversión válida.
- Las operaciones potencialmente irreversibles requieren estrategia de recuperación y autorización pertinente.

## Desarrollo incremental
- Delimitar un resultado observable y sus criterios de aceptación.
- Reutilizar lógica existente cuando sea válida.
- Cambios pequeños y coherentes: interfaz, lógica, persistencia y errores cuando correspondan.
- Evitar añadir infraestructura especulativa, funciones ficticias o iconos sin acción real.
- Distinguir explícitamente implementado, parcialmente implementado, documentado y probado.
- Registrar decisiones estables; no documentar cada movimiento ni producir informes ceremoniales.

## Verificación proporcional al riesgo
- Nivel 0: documentación; revisar diff, formato y referencias, sin compilar por defecto.
- Nivel 1: cambios visuales; revisar pantalla afectada y sintaxis cuando proceda.
- Nivel 2: lógica localizada; pruebas unitarias y casos límite relacionados.
- Nivel 3: persistencia/archivos/transferencias/APIs; integración y errores del flujo afectado.
- Nivel 4: recuperación, migración, seguridad, entrega; probar reversibilidad y regresión suficiente.
- Comandos del crate, cuando el entorno Rust esté disponible: cd app && cargo fmt --check; cargo test; cargo check.
- Elegir solo pruebas pertinentes al cambio; no ejecutar la suite completa automáticamente tras cada edición.
- No afirmar pruebas de hardware, integración externa o compilación si no se han ejecutado en el estado revisado.
- No repetir pruebas ya válidas sin cambios relevantes.
- Si una prueba falla dos veces con la misma causa, cambiar el enfoque de diagnóstico.

## Git y documentación
- Aplicar cambios directamente sobre main cuando estén autorizados y no sean críticos.
- Antes de escribir, verificar que la ruta no existe o recuperar su SHA si se actualiza.
- Los commits deben ser descriptivos, revisables y limitados al alcance.
- Revisar contenido escrito y estado final de main; no atribuir éxito a una operación no verificada.
- No modificar app/library.json como efecto secundario de una auditoría.
- No crear ramas, PR, issues o despliegues sin aportar un beneficio real a la tarea.
- Si la tarea solo exige análisis, conservar el código intacto y dejar registro documental únicamente cuando sea útil.
- Los problemas relevantes no corregidos deben permanecer visibles en la documentación.

## Comunicación y continuidad
- Entregar resultado, archivos cambiados, evidencias, pruebas no realizadas, riesgos y commit/estado Git.
- Indicar con claridad dependencias externas y limitaciones de plataforma.
- Si hay decisión funcional importante, presentar opciones y consecuencias en el debate.
- Preservar la utilidad de la app actual; ampliar funcionalidades sin sacrificar fiabilidad.
- Cada operación debe comprender, implementar, verificar, proteger o entregar valor.
