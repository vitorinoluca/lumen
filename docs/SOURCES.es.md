# Fuentes de datos

lumen analiza todo de forma local. El único uso de internet es descargar datos públicos y gratuitos, y solo
cuando pedís una revisión. Cada fuente se revisa (licencia y términos) antes de integrarla. Una fuente que
exige registro o una clave se descarta.

## Usadas

| Fuente | Licencia | Qué usa lumen | Cuándo se descarga |
|---|---|---|---|
| [LOLDrivers](https://www.loldrivers.io) | Apache-2.0, descarga libre, sin clave | Hashes SHA-256 de drivers vulnerables o maliciosos conocidos, con su categoría. Solo se guardan los hashes (unos 160 KB) en `%APPDATA%\lumen\db`. | Durante "Revisar mi PC", solo si la lista falta o tiene más de una semana. Es el único momento en que lumen usa internet. |

Atribución: este producto contiene datos de LOLDrivers (https://github.com/magicsword-io/LOLDrivers),
con licencia Apache License 2.0.

## Evaluadas y descartadas

| Fuente | Motivo |
|---|---|
| abuse.ch MalwareBazaar | Su API exige una `Auth-Key` (registro) y funciona bajo "uso justo", con una suscripción paga posible para uso comercial. No es CC0. |

## Pendientes de revisión

Reglas YARA (por ejemplo YARA-Forge) y firmas de ClamAV: primero se revisa la licencia, como pide el plan.
