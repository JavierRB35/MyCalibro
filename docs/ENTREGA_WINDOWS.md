# MyCalibro — construcción y evidencia de entrega Windows

Estado a 23/11/2026: **desarrollo en curso; no es la entrega funcional final**.
Coordinación y reservas de archivos: [issue #1](https://github.com/JavierRB35/MyCalibro/issues/1).
Las decisiones de producto siguen en [DECISIONES_PRODUCTO.md](DECISIONES_PRODUCTO.md).

## Construir un instalador

Requisitos de construcción: Rust estable, toolchain MSVC para Windows x64 y NSIS 3.
Desde la raíz del repositorio, en PowerShell:

```powershell
cargo test --manifest-path app/Cargo.toml --locked
cargo build --manifest-path app/Cargo.toml --release --locked
New-Item -ItemType Directory -Path dist -Force
& "${env:ProgramFiles(x86)}\NSIS\makensis.exe" /DVERSION=0.1.0 packaging/windows.nsi
```

El instalador queda en `dist/MyCalibro-0.1.0-setup.exe`. Instala por usuario en
`%LOCALAPPDATA%\Programs\MyCalibro`, con accesos en Inicio. Los datos están fuera
del directorio de instalación. La desinstalación elimina sus binarios y accesos,
sin borrar bibliotecas, preferencias ni copias. Las actualizaciones son manuales;
no hay actualizador automático ni firma Authenticode configurada.

Calibre sigue siendo una dependencia externa para conversiones con `ebook-convert`
y extracción con `ebook-meta`. Un error de dependencia debe mostrarse en la aplicación,
sin presentar una conversión o lectura de metadatos como completada.

## Validación del instalador sin tocar una instalación real

```powershell
./scripts/windows-smoke.ps1 -Installer ./dist/MyCalibro-0.1.0-setup.exe
```

El script crea un directorio único, usa `/TESTMODE` para evitar registro/accesos
de instalaciones existentes y aísla `LOCALAPPDATA`. Comprueba instalación, ejecución
del binario instalado, captura de la ventana, hash del catálogo antes/después y
desinstalación que conserva los datos. Guarda captura, stderr y `result.json`.
Con `_?=` NSIS puede dejar el stub del desinstalador; la prueba exige que se elimine
el ejecutable de MyCalibro y se conserven los datos.

La acción [Windows candidate verification](../.github/workflows/windows.yml) se
ejecuta manualmente o con etiquetas `v*`. Produce instalador, SHA-256 y evidencias.
`run_tests=false` solo corresponde a comprobaciones de empaquetado con pruebas
Windows pertinentes ya registradas. Un candidato funcional final necesita validación
integrada; compilar el instalador no acredita sus flujos.

## Evidencia registrada

| Fuente | Resultado y límite |
| --- | --- |
| [PR #2](https://github.com/JavierRB35/MyCalibro/pull/2), e881e37 | P0 integrado: 18 pruebas Linux y 17 Windows; catálogo corrupto preservado, bloqueo, recuperación de JSON y transferencias. Revisión cruzada de B aprobada. |
| [Run 45160690573](https://github.com/JavierRB35/MyCalibro/actions/runs/45160690573), f765fb3 | Release e instalador generados. El runner no dispone de OpenGL 2.0; falló el arranque allí. |
| [Comprobación de B en Windows real](https://github.com/JavierRB35/MyCalibro/issues/1#issuecomment-7099328433), instalador f765fb3 | Instalación aislada, ventana visible, catálogo intacto y desinstalación con datos preservados. B completó manualmente los pasos tras detectar un fallo de captura de ExitCode en el script, posteriormente corregido. Valida ese artefacto P0, no funciones posteriores. |
| [Run 45161919605](https://github.com/JavierRB35/MyCalibro/actions/runs/45161919605), 8c57e95 | Pruebas Windows, incluida credencial ficticia nativa, y construcción correctas. Arranque falló porque faltaba habilitar dx12 en wgpu. |
| [Run 45163682512](https://github.com/JavierRB35/MyCalibro/actions/runs/45163682512), e301d4c | Todos los pasos correctos: pruebas, release, NSIS, instalación, ventana capturada, catálogo intacto y desinstalación con datos preservados. SHA-256 del instalador: `3d19395d8a56e7cdc6a505d3badea2832f7b14e655e5db95e42ae379c3877238`. |
| [Run 45170386951](https://github.com/JavierRB35/MyCalibro/actions/runs/45170386951), 65f99e5 | 67/67 pruebas Windows, incluida renderización nativa de PDF real de dos páginas, release, instalador y smoke correctos. Incluye almacenamiento, respaldo y asistente/Calibre integrados. SHA-256 del instalador: `7df442d14682efa4545c9a9d3a00f7419d9eb41da92eedbadd537332e4214f09`. [Artefacto y evidencias](https://github.com/JavierRB35/MyCalibro/actions/runs/45170386951/artifacts/14816517765). No acredita todavía conexión visual del lector ni análisis IA. |

Las PR [#3](https://github.com/JavierRB35/MyCalibro/pull/3) y
[#4](https://github.com/JavierRB35/MyCalibro/pull/4) están fusionadas tras revisión
de B: biblioteca híbrida, respaldo/restauración e importación revisada desde
archivos, carpetas y Calibre. El backend PDF nativo ya se ejecutó en Windows;
`pdf_view.rs` aporta una ventana independiente pendiente de conectar a las fichas.

## Pendientes para declarar finalización

- Revisión e integración de [PR #5](https://github.com/JavierRB35/MyCalibro/pull/5)
  (metadatos/portada EPUB nativos) y [PR #6](https://github.com/JavierRB35/MyCalibro/pull/6)
  (recuperación/exportación JSON que conserva recursos administrados relativos).
- Conectar y comprobar el lector EPUB/PDF y el análisis IA por libro con
  consentimiento revocable. Módulos aislados o pruebas unitarias no acreditan el flujo completo.
- Ejecutar la validación del instalador sobre el candidato integrado, registrar
  sus resultados y publicar la versión de entrega. No hay evidencia de hardware USB
  ni llamadas reales a proveedores IA en las pruebas automáticas aquí enumeradas.
