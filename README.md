# MyCalibro

Biblioteca de escritorio sencilla («Mis libros») hecha en Rust con eframe/egui, construida como versión propia y simplificada de Calibre. Permite buscar libros, filtrar por tema o etiqueta, añadir y editar fichas, importar ebooks arrastrándolos a la ventana, clasificarlos automáticamente con IA (Gemini/Groq) y enviarlos a lectores conectados (Kindle, Kobo, NOOK) usando Calibre para metadatos y conversión.

> **Estado:** aplicación en desarrollo (no acabada). Este repositorio es la única fuente de verdad del proyecto.

## Estructura del repositorio

```
.
├── app/                  # Aplicación (crate Rust)
│   ├── src/main.rs       # Código fuente completo de la app
│   ├── Cargo.toml        # Manifiesto y dependencias
│   ├── Cargo.lock        # Versiones bloqueadas de dependencias
│   ├── README.md         # Cómo ejecutar la aplicación
│   ├── MECHANICS.md      # Mecánicas investigadas del Calibre original
│   ├── mechanics.json    # Valores de mecánicas con su fuente
│   ├── library.json      # Datos de la biblioteca (la app guarda aquí)
│   └── shots/            # Capturas generadas por la propia app
├── capture/              # Análisis del ejecutable original de Calibre
├── checks/               # Capturas de las verificaciones de compilación
├── inbox/                # Progreso de construcción
├── investigation.cany/   # Evidencia, diario y hallazgos de la investigación
├── pinned/               # Registros de ejecución
├── craft.json            # Configuración del proyecto
├── feed.json             # Bitácora de eventos del proyecto
├── questions.json        # Preguntas abiertas sobre el original
└── usage.json            # Registro de uso
```

## Ejecutar la aplicación

Requiere [Rust](https://rustup.rs). Desde la carpeta `app/`:

```bash
cargo run
```

Los cambios se guardan automáticamente en `app/library.json`. Para más detalles (importación con IA, capturas sin foco, lectores compatibles), consulta [`app/README.md`](app/README.md).
