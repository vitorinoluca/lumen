# lumen

[English](README.md) | **Español**

Optimizador de PC para Windows, liviano y honesto. Un solo `.exe` portable (~8 MB), ~6 MB de RAM privada, abre en ~35 ms.
Sin telemetría, sin cuentas, sin servicios pagos.

## Qué hace (v0.5)
Cada sección muestra las opciones con casilla, una explicación corta y una etiqueta de riesgo (verde = seguro, amarillo = revisar). Lo amarillo viene desmarcado y lo avanzado queda detrás de "Más opciones". Antes de aplicar, lumen muestra exactamente qué va a hacer, crea un punto de restauración de Windows (una vez por sesión) y guarda el valor original de cada ajuste. Pide permisos de administrador al abrir.
- **Inicio:** el estado de tu PC (espacio para liberar, apps de inicio, memoria), un botón "Analizar" y tres presets: Mínimo, Recomendado y Avanzado. Debajo, tu hardware con CPU y GPU en vivo.
- **Limpieza:** temporales, cachés de navegadores y restos de Windows Update van a una reserva por 7 días (se puede deshacer); buscador de duplicados; y analizador de disco con barras.
- **Rendimiento:** plan de energía, modo juego, apps de inicio (con su ícono), servicios que casi nadie usa, efectos visuales y apps en segundo plano.
- **Privacidad:** telemetría, publicidad, sugerencias del menú Inicio y quitar apps preinstaladas (esto último no se puede deshacer).
- **Red:** limpiar la caché de DNS, resetear la red y elegir DNS (Cloudflare, Google o automático).
- **Reparación:** SFC, DISM, reparar Windows Update y salud del disco (SMART).
- **Actualizaciones:** por defecto, solo seguridad o pausadas.
- **Historial:** todos los cambios que hizo lumen, con un botón para deshacer cada uno.
- **Más:** procesos (tocá uno para cerrarlo), programas (abre el desinstalador y limpia restos), seguridad (Windows Defender y una revisión de comportamiento que detecta lo que puede pasar por alto) y configuración (tema, frecuencia del panel, español o inglés).
lumen no instala programas, no tiene "limpiador de registro" ni "liberador de RAM".

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
