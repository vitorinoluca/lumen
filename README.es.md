# lumen

[English](README.md) | **Español**

Optimizador de PC para Windows, liviano y honesto. Un solo `.exe` portable (~8 MB), ~6 MB de RAM privada, abre en ~35 ms.
Sin telemetría, sin cuentas, sin servicios pagos.

## Qué hace (v0.1)
- **Estado:** CPU, memoria, disco y red en vivo.
- **Procesos:** lista ordenable con ruta y botón para cerrar un proceso (pide confirmación).
- **Arranque:** activa o desactiva apps de inicio sin borrarlas (igual que el Administrador de tareas).
- **Disco:** limpia temporales y cachés de Chrome/Edge, y muestra qué ocupa más espacio.
- **Privacidad:** apaga el ID de publicidad, las sugerencias, Bing en la búsqueda de Inicio, la telemetría y el historial de actividades.

En Windows 11 la ventana es translúcida (Acrylic). En otros sistemas usa un fondo sólido.

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

## Licencia
MIT
