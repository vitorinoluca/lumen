# Plan de lumen

[English](PLAN.md) | **Español**

App de escritorio para Windows que monitorea, limpia, optimiza y revisa la seguridad de la PC.
Open source, honesta (sin humo), portable, rápida y con un consumo mínimo de recursos.

## Principios (no negociables)
1. **Liviano:** < 20 MB de RAM en reposo, ejecutable < 15 MB, arranque < 300 ms. El CI lo verifica.
2. **Reversible:** toda acción que modifica el sistema se registra y se puede deshacer.
3. **Transparente:** cada acción explica qué hace y su riesgo antes de ejecutarse.
4. **Sin humo:** nada de "limpiador de registro" ni "liberador de RAM".
5. **Privado y gratuito:** cero telemetría, cero API keys, cero servicios pagos. El análisis es local; la única red es la descarga opcional de firmas y reglas públicas gratuitas.
6. **Sin procesos en segundo plano** salvo que el usuario active el monitor.

## Stack
Rust, Slint (renderizador por software), `sysinfo`, `winreg`, `windows-sys`. Persistencia en JSON en `%APPDATA%\lumen`.
Build con `opt-level = "z"`, `lto`, `strip` y `panic = "abort"`. CI en GitHub Actions.

## Arquitectura
```
crates/core   lógica sin UI: monitor, procesos, inicio, disco, privacidad, actions (journal)
crates/ui     Slint, solo presentación
```
Toda modificación del sistema pasa por `actions` (`apply()` / `revert()`). Las lecturas pesadas van en hilos de fondo.

## Fases

### Fase 0 — Base (hecha)
- [x] Workspace de Cargo y CI con build, clippy y tests.
- [x] Ventana Slint con navegación lateral.
- [x] Medición de RAM y arranque en CI.
- [x] Journal de acciones con apply/revert y tests.

### Fase 1 — MVP (hecha, v0.1)
- [x] Monitor: CPU, RAM, disco y red.
- [x] Procesos: lista ordenable, ruta y finalizar.
- [x] Inicio: activar/desactivar reversible.
- [x] Disco: temporales, cachés de Chrome/Edge, mapa de espacio.
- [ ] Pendiente: GPU, refresco configurable, firma digital (va con la Fase 3), papelera y Firefox.

### Fase 2 — Optimización (hecha, v0.2)
- [x] Privacidad: ajustes de Windows reversibles.
- [x] Desinstalador con limpieza de restos.
- [x] Servicios con base curada de "seguro de desactivar".
- [x] Perfiles: modo juego y modo batería.
- [x] Punto de restauración automático antes de cambios grandes (por ahora al desactivar servicios; el desinstalador y los perfiles también lo usarán).

### Fase 3 — Revisión de seguridad local y gratuita (en curso)
- [x] Capa 1: estado, detecciones y análisis rápido de Windows Defender.
- [x] Revisión de comportamiento (línea base + diferencias): persistencia, reverse shells, comandos ofuscados, manipulación de Defender y Windows Update, procesos de sistema falsos. Acciones: ignorar, deshabilitar con respaldo, restaurar.
- [ ] Hashes conocidos, reglas YARA, ClamAV opcional y el resto de las heurísticas.
Detección en capas, todo offline: Windows Defender, hashes conocidos (abuse.ch, LOLDrivers), reglas YARA (`yara-x`),
ClamAV opcional (descarga aparte, GPL) y heurísticas propias con puntaje. Cuarentena reversible.
Texto fijo: "Revisión de seguridad, no reemplaza a un antivirus". Nunca afirmar "PC limpia".
Verificar la licencia de cada fuente antes de integrarla. Nunca manejar malware real fuera de una VM.

### Fase 4 — Salud y pulido
SMART, batería, temperaturas, drivers (solo informar), idioma español/inglés, ejecutable firmado, documentación.

## Riesgos
| Riesgo | Mitigación |
|---|---|
| Falsos positivos de antivirus sobre la app | Firmar el ejecutable, código abierto, builds en CI |
| Romper el sistema del usuario | Journal con deshacer, punto de restauración, confirmación |
| Desactivar servicios críticos | Base curada; los críticos no se pueden tocar |
| Prometer seguridad que no hay | Lenguaje de "señales de alerta", nunca "limpio" |
| Hinchazón de funciones | Medición de RAM y arranque en CI que falla al pasarse |

## Reglas de desarrollo
- No agregar dependencias pesadas sin justificarlo (mirar el tamaño del binario).
- Toda función que modifique el sistema va en `core/actions` con su `revert()` y sus tests.
- Nada de API keys ni servicios pagos. Nunca subir archivos del usuario a ningún lado.
- `cargo clippy --all-targets -- -D warnings` y los tests antes de cada commit.
- Commits cortos, uno por funcionalidad.
