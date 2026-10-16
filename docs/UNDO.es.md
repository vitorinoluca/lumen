# Cómo deshacer cambios

lumen guarda cada cambio en `%APPDATA%\lumen\journal.json`, así que se puede deshacer incluso después de reiniciar la app.

| Página | Cómo deshacer |
|---|---|
| Arranque | Volvé a mover el interruptor. No se borra nada; lumen solo marca la entrada como desactivada. |
| Privacidad | Volvé a mover el interruptor. lumen restaura el valor que tenía Windows antes (o lo quita si no existía). |
| Disco | Los archivos se mueven a `%APPDATA%\lumen\stash`, no se borran. "Deshacer última limpieza" los devuelve. |

## Lo que no se puede deshacer
- **Vaciar reserva** borra los archivos definitivamente. Recién ahí se libera el espacio.
- **Cerrar un proceso** pierde lo que ese programa no hubiera guardado.

## Notas
- Lo marcado como "administrador" (inicio del equipo, telemetría) necesita que lumen corra como administrador. Sin eso, el cambio falla con un mensaje y no se modifica nada.
