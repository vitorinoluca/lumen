# lumen

Optimizador de PC para Windows, liviano y honesto. Un solo `.exe` portable (~8 MB), ~11 MB de RAM, abre en ~35 ms.
Sin telemetría, sin cuentas, sin servicios pagos.

## Qué hace (v0.1)
- **Monitor:** CPU, RAM, disco y red en vivo.
- **Procesos:** lista ordenable con ruta y botón para finalizar (pide confirmación).
- **Inicio:** activa/desactiva apps de inicio sin borrarlas (igual que el Administrador de tareas).
- **Disco:** limpia temporales y cachés de Chrome/Edge, y muestra el espacio por carpeta.

## Cómo se deshace todo
Cada cambio se guarda en `%APPDATA%\lumen\journal.json`.
- Inicio: volvé a mover el interruptor.
- Limpieza: los archivos van a `%APPDATA%\lumen\stash`, no se borran. "Deshacer última limpieza" los devuelve.
  El espacio se libera al "Vaciar reserva", y **eso no se puede deshacer**.
- Finalizar un proceso tampoco se puede deshacer.

## Lo que NO hacemos
Limpiador de registro ni "liberador de RAM": no sirven y pueden romper cosas.

## Desarrollo
```
cargo run --release                  # abrir la app
cargo clippy --all-targets -- -D warnings && cargo test
powershell scripts/check-limits.ps1  # tamaño, RAM y arranque (lo corre el CI)
```

## Licencia
MIT
