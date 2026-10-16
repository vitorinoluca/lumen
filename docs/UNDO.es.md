# Cómo deshacer cambios

lumen guarda cada cambio en `%APPDATA%\lumen\journal.json`, así que se puede deshacer incluso después de reiniciar la app.

| Página | Cómo deshacer |
|---|---|
| Arranque | Volvé a mover el interruptor. No se borra nada; lumen solo marca la entrada como desactivada. |
| Privacidad | Volvé a mover el interruptor. lumen restaura el valor que tenía Windows antes (o lo quita si no existía). |
| Servicios | Volvé a mover el interruptor. lumen restaura el tipo de inicio que tenía el servicio. Rige después de reiniciar. |
| Perfiles | Apagá el perfil: vuelve el plan de energía anterior. Las apps que cerró el modo juego no se reabren. |
| Programas | Las carpetas que sobran van a la reserva y las claves de registro se respaldan en un `.reg`. "Deshacer limpieza de restos" restaura ambas. La desinstalación del programa en sí no se deshace (usá el punto de restauración). |
| Seguridad | "Deshabilitar" guarda un respaldo antes y pasa por el journal; "Restaurar" lo deja exactamente como estaba. "Ignorar" solo agrega el elemento a la línea base (no cambia nada en tu PC) y se deshace borrando `%APPDATA%\lumen\security-baseline.json`. Los respaldos de tareas programadas quedan en `%APPDATA%\lumen\backups`. |
| Drivers | No hay nada que deshacer: la revisión de drivers solo lee archivos. La base descargada está en `%APPDATA%\lumen\db` y se puede borrar. |
| Disco | Los archivos se mueven a `%APPDATA%\lumen\stash`, no se borran. "Deshacer última limpieza" los devuelve. |

## Lo que no se puede deshacer
- **Vaciar reserva** borra los archivos definitivamente. Recién ahí se libera el espacio.
- **Cerrar un proceso** pierde lo que ese programa no hubiera guardado.

## Puntos de restauración
Antes de desactivar un servicio, lumen le pide a Windows un punto de restauración (necesita administrador y la Protección del sistema activada). Windows crea como máximo uno cada 24 horas. Si no se puede crear, lumen te avisa y aplica el cambio igual, que se puede deshacer desde lumen.

## Notas
- Lo marcado como "administrador" (inicio del equipo, telemetría) necesita que lumen corra como administrador. Sin eso, el cambio falla con un mensaje y no se modifica nada.
