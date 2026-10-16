# lumen

[English](README.md) | **Español**

Optimizador de PC para Windows, liviano y honesto. Un solo `.exe` portable (~8 MB), ~6 MB de RAM privada, abre en ~35 ms.
Sin telemetría, sin cuentas, sin servicios pagos.

## Qué hace (v0.2)
- **Panel:** tu hardware (Windows, CPU, RAM, GPU, placa, discos) con CPU, memoria y red en vivo.
- **Procesos:** lista ordenable con ruta y botón para cerrar un proceso (pide confirmación).
- **Arranque:** activa o desactiva apps de inicio sin borrarlas (igual que el Administrador de tareas).
- **Limpieza de disco:** un botón libera temporales y cachés de Chrome/Edge. Los archivos van a una reserva por 7 días, así se puede deshacer.
- **Optimizar:** una lista simple (modos juego y batería, privacidad, servicios que casi nadie usa) y un botón "Aplicar las recomendadas". Lo delicado queda en "avanzadas" y los servicios críticos nunca se muestran.
- **Programas:** abre el desinstalador de cada app y después revisás y limpiás carpetas y claves de registro que hayan quedado.
- **Seguridad:** estado y detecciones de Windows Defender, y una revisión de comportamiento que detecta lo que Defender puede pasar por alto: entradas nuevas de inicio, tareas y servicios, cambios en Winlogon/IFEO/WMI, reverse shells, comandos ofuscados, procesos de sistema falsos y Defender o Windows Update desactivados. El primer escaneo guarda una línea base; los siguientes avisan solo de lo nuevo o modificado.
- **Configuración:** tema claro, oscuro o el de Windows, frecuencia del panel e idioma español o inglés. Se guarda en `%APPDATA%lumennfig.json`.

## Cómo se deshace todo
Cada cambio se guarda en `%APPDATA%\lumen\journal.json`. Ver [docs/UNDO.es.md](docs/UNDO.es.md).

## Lo que NO hacemos
Limpiador de registro ni "liberador de RAM": no sirven y pueden romper cosas.

## Compilar
```
cargo run --release                  # abrir la app
cargo clippy --all-targets -- -D warnings && cargo test
powershell scripts/check-limits.ps1  # límites de tamaño, RAM y arranque (lo corre el CI)
```
Requiere Rust y las herramientas de compilación de MSVC.

## Plan
Ver [PLAN.es.md](docs/PLAN.es.md). Reglas de diseño: [docs/DISENO.md](docs/DISENO.md).

## Fuentes de datos
La revisión de drivers usa LOLDrivers (Apache-2.0), que se descarga durante una revisión y se renueva como mucho una vez por semana. Ver [docs/SOURCES.es.md](docs/SOURCES.es.md).

## Licencia
MIT
