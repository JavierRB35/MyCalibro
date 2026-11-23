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
| [Run 45163682512](https://github.com/JavierRB35/MyCalibro/actions/runs/45163682512), e301d4c | Verificación en curso tras habilitar dx12 y corregir el handle del proceso. Consultar resultado antes de atribuir éxito. |

## Pendientes para declarar finalización

- Revisión e integración de [PR #3](https://github.com/JavierRB35/MyCalibro/pull/3):
  almacenamiento híbrido, preferencias y respaldo/restauración. Pruebas Linux
  registradas en el PR; revisión Windows y funcional de B pendiente.
- [PR #4](https://github.com/JavierRB35/MyCalibro/pull/4): backend Calibre de solo
  lectura, con tres pruebas de origen intacto. Falta revisión e integración con el asistente.
- Conectar y comprobar el asistente de importación, el lector EPUB/PDF y la IA con
  consentimiento revocable. Módulos aislados o pruebas unitarias no acreditan el flujo completo.
- Ejecutar la validación del instalador sobre el candidato integrado, registrar
  sus resultados y publicar la versión de entrega. No hay evidencia de hardware USB
  ni llamadas reales a proveedores IA en las pruebas automáticas aquí enumeradas.
