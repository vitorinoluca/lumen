# Lumen — Plan de rediseño de la UI ("Calma y control")

Estado: **borrador para aprobación**. No se tocó código. Alcance: solo la capa visual (`crates/ui/ui/main.slint` y, si hace falta, el pegamento de `crates/ui/src`). `crates/core` no se toca.

---

## 0. Auditoría (Fase 1)

### 0.1 Stack y organización
- **Slint 1.18.1**, renderer **software** (`renderer-software`, sin GPU, sin Skia/femtovg), backend winit 0.30 en Windows. Sin webview, sin JS.
- Toda la UI está en **un solo archivo**: `crates/ui/ui/main.slint` (1529 líneas). Rust (`main.rs` 872 l., `sections.rs` 447 l., `duplicates.rs` 110 l.) solo hace de pegamento: `ui.set_*()` y callbacks.
- Tema actual: `global Theme` con colores **translúcidos sobre Mica** (blanco/negro con alfa), `Segoe UI Variable Text` del sistema, ícono propio `Icon` hecho con `Path` (sin SVG).
- Textos: `@tr("" => "…")` con el español como msgid + `lang/en/lumen.po` (1352 líneas). Cambiar la redacción de un texto = actualizar el `.po`.
- Presupuesto del proyecto (`scripts/check-limits.ps1`, corre en CI): **exe < 15 MB, RAM < 20 MB, arranque < 300 ms**. Hoy el exe pesa **9,3 MB** (release del 16/10; la línea base de RAM y arranque se mide en la etapa 0, necesita consola de administrador).

### 0.2 Pantallas, modales y estados existentes

| Página (`page`) | Qué es hoy | Estados que existen |
|---|---|---|
| 0 Inicio | 3 métricas (liberar / apps al iniciar / memoria), bloque Analizar→presets→Optimizar, CPU/GPU, discos, ficha del equipo | `home-state` 0 sin analizar, 1 analizando (barra sin fin), 2 listo, 3 aplicado |
| 3 Limpieza | 3 pestañas: Limpiar, Duplicados, Espacio | `disk-phase` 0–3, `disk-loading`, `disk-nothing`, `dup-phase` 0–3, vacío de duplicados, `space-map` vacío |
| 1 Procesos | tabla ordenable, **clic en la fila = cerrar** | solo lista (sin vacío ni error) |
| 7 Programas | lista + búsqueda + orden + restos tras desinstalar | `status`, restos, deshacer |
| 8 Seguridad | botón circular → revisión de **comportamiento** (Defender, arranque, conexiones, procesos, drivers) | `behavior-run`, `behavior-busy` (progreso = 1 línea de texto), resultados, `sec-status` |
| 10–14 Rendimiento, Privacidad, Red, Reparación, Actualizaciones | **una sola plantilla** de opciones con casillas + riesgo + vista previa | `sc-loading`, `sc-busy`, `sc-result`, `sc-preview`, `done` por fila |
| 15 Historial | lista con Deshacer por cambio | vacío, `hist-status` |
| 9 Configuración | tema, refresco, idioma | — |
| Modales | `Confirm` (desinstalar, cerrar proceso, hallazgos), `PreviewDialog` (qué se va a hacer) | — |

Faltan por completo: toasts, tooltips, skeletons, estado de **error** (los errores salen como texto suelto en `status`/`*-result`), foco de teclado.

### 0.3 Problemas concretos

**Jerarquía / densidad**
1. El menú tiene **11 entradas** sin etiquetas de grupo (solo un hueco de 16 px). Privacidad/Red/Reparación/Actualizaciones compiten con Inicio y Limpieza.
2. Inicio mezcla **tres jerarquías**: métricas, un bloque con 4 estados y una ficha técnica larga (Sistema/Procesador/Gráficos/Placa/Red). Se pierde la regla de "una acción principal": hay que bajar para ver discos.
3. Seguridad promete mucho y muestra poco: sin progreso numérico, sin cancelar, sin tipos de escaneo, sin cuarentena visible (la cuarentena existe en el core como `Action::MoveFile` del diario, pero la UI no la expone).

**Consistencia**
4. La tarjeta-fila "66 px de caja / 60 px de tarjeta / radio 8" está **copiada 5 veces** (Limpieza, Duplicados, Opciones, Historial, + variantes). Radios mezclados: 4, 5, 6, 8, 10, 12 px. Alturas mágicas: 24, 26, 28, 30, 32, 36, 38, 44, 52, 58, 64, 66, 72, 120, 134.
5. `Btn` se instancia con `height: 28/30/32/36/38` y `pad: 14/20/28` a mano. No hay tamaños definidos.
6. **Alturas de contenido calculadas a mano** para el scroll (`content-h: 778px + disks.length * 64px`, `finding_height()` en Rust, `sc-h`, `rows-h`). Cualquier cambio de espaciado rompe el scroll en silencio (el propio código lo advierte).
7. Estilos hardcodeados: **91** `font-size: Npx` literales (solo 4 valores distintos, pero sin token), 16 colores hex fuera de `Theme` (scrim `#00000090`, `#00000050`), `border-radius` literal en ~25 sitios, `spacing`/`padding` literales en prácticamente todos los layouts.

**Contraste / color**
8. Con Mica, el texto es **blanco/negro con alfa** (`muted #ffffff9e`, `faint #ffffff5c`). Su contraste real depende del fondo de escritorio del usuario: **no se puede garantizar AA**. `faint` (≈36 % de opacidad) casi seguro no lo cumple.
9. **La fila de Procesos se pone roja entera al pasar el mouse** (`danger.with-alpha(0.3)`). Es alarmismo gratuito sobre una acción que ni se hizo: contradice el principio 3.
10. Rojo usado como borde de severidad "Alta" en hallazgos de una revisión de *comportamiento* (no son amenazas confirmadas).

**Tipografía**
11. Segoe UI Variable depende del sistema (Windows 10 no la trae completa). No hay fuente monoespaciada para rutas/hashes: las rutas de Procesos y las líneas de comando decodificadas salen en la fuente de UI. Sin tabular-nums: las cifras "bailan" en CPU/RAM que se actualizan cada 1–5 s.

**Navegación / feedback / accesibilidad**
12. **Cero soporte de teclado**: no hay `FocusScope`, ni foco visible, ni Tab; todo es `TouchArea`. Atajos: ninguno.
13. El botón **Desinstalar solo aparece al pasar el mouse** (`visible: app-ta.has-hover`): invisible para teclado y táctil.
14. Cargas largas con `Busy` (barra sin fin) donde sí se conoce el progreso o al menos el paso. No hay forma de cancelar nada.
15. Sin tooltips: las etiquetas Seguro/Revisar y los íconos no se explican.
16. Sin roles de accesibilidad (`accessible-role`, `accessible-label`) en ningún componente propio.

### 0.4 Limitaciones del framework (y cómo se resuelven)

| Limitación (Slint 1.18, renderer software) | Impacto | Resolución |
|---|---|---|
| **Sin sombras difusas ni blur** (`drop-shadow` no existe en el renderer software) | No hay elevación "real" | Elevación = capas de grafito más claras + borde sutil + scrim opaco para modales. Coincide con "sombras mínimas". |
| **Sin rotación de formas** | Un spinner no puede girar | Ya se resuelve hoy recalculando el arco del `Path` con `animation-tick()`; se mantiene. El anillo de progreso usa el mismo recurso con un valor real. |
| **Sin `font-feature-settings`** (no se puede pedir `tnum`) | "Tabular-nums" no es una propiedad | **Congelar `tnum` dentro del archivo de Inter** con `pyftfeatfreeze` al preparar la fuente (una vez, offline). Los dígitos quedan tabulares en toda la app. Costo: los números dentro de prosa se ven apenas más anchos. |
| Fuentes variables | Hay que confirmarlo | Se comprobó en el código de `i-slint-renderer-software` que el renderer maneja variaciones de eje (`fonts/vectorfont.rs`). Se valida con un prototipo en la etapa 2 antes de seguir. |
| Íconos Lucide son SVG | Cargar SVG exige el feature `svg` (resvg), pesado | **No se agrega ninguna dependencia.** Se convierten ~35 íconos de Lucide a cadenas `Path` (como hoy) con un script único; los `circle/rect/line` pasan a comandos de path. Licencia ISC: se incluye el aviso. |
| Sin variables CSS | Tokens | `global Tokens` con `out property` calculadas según `Tokens.dark`; todo valor visual sale de ahí. |
| `TouchArea` no toma foco | Teclado | Cada componente interactivo envuelve un `FocusScope` + anillo de foco + Enter/Espacio. Se centraliza en 4 componentes base (Button, Toggle/Check, NavItem, Row). |
| Ventana translúcida (Mica) rompe repintado parcial con `clip` redondeado | Artefactos ya documentados en `docs/DISENO.md` | Ver decisión D1: pasar a fondo **opaco** elimina el problema y garantiza el contraste. |
| `prefers-reduced-motion` no existe | Animaciones | Se agrega una opción en Configuración ("Reducir animaciones"), ver D4. |

---

## 1. Decisiones que necesito de vos antes de implementar

| # | Decisión | Mi recomendación |
|---|---|---|
| **D1** | Tu dirección pide *superficies grafito opacas en capas*. La app hoy usa **Mica translúcido** (ver `docs/DISENO.md`). Son incompatibles. | **Pasar a grafito opaco.** Garantiza contraste AA (con Mica es imposible), elimina el bug de repintado, y permite borrar el camino `solid`/`set_system_backdrop`. Se pierde el "efecto Windows 11". Cambio mínimo en Rust: forzar `solid = true` (luego se borra el código muerto, 8 líneas en `apply_theme`). |
| **D2** | **Navegación**: hoy 11 entradas planas. | Agrupar en **7 + Configuración** usando pestañas internas (ver §3.2). Las páginas numéricas de Rust no cambian: la pestaña solo asigna `page`. **Cero cambios en lógica.** |
| **D3** | Tema por defecto: hoy `Automático`. Pedís oscuro por defecto. | Nuevas instalaciones en **Oscuro**; Automático sigue disponible. Implica cambiar el default de `config.rs` (una línea). |
| **D4** | "Reducir animaciones": ¿lo agrego a Configuración? | Sí, un toggle; desactiva el giro/barra sin fin y deja un indicador estático. Requiere un campo nuevo en `config.rs`. |

### Cambios de lógica que **propongo y NO implemento sin tu OK** (justificados por la UX)

| Propuesta | Por qué | Costo |
|---|---|---|
| L1. **Cancelar** el escaneo de Seguridad y el análisis de Inicio | Principio 1/2: el usuario siempre manda; hoy no hay salida de un escaneo largo | Un `AtomicBool` que el core ya podría chequear en su bucle |
| L2. **Progreso numérico** (archivos / total o %) en Seguridad, Duplicados, Limpieza | Hoy solo hay texto o barra sin fin; el anillo de progreso necesita un valor | El callback de progreso ya existe (`ui.set_progress`); se agregaría un `f32` |
| L3. **Tipos de escaneo** (rápido / completo / personalizado) y **cuarentena visible** con restaurar/eliminar | Es lo que describe tu brief; hoy la UI solo expone la revisión de comportamiento. El motor de firmas/hashes (`hashdb.rs`, `signature.rs`) y el movimiento a cuarentena existen en el core, pero no están conectados a ninguna pantalla | Mediano: es una pantalla nueva con lógica nueva. **Fuera del alcance visual**; el plan deja el hueco diseñado |
| L4. **Impacto de arranque** (alto/medio/bajo) y toggle de programas de inicio | La pantalla "Gestor de inicio" se eliminó en el commit `d5f0312`; el core conserva `startup.rs` | Reexponer lo que ya hay en el core |
| L5. **Fecha de última revisión de seguridad** guardada | Para la tarjeta "Seguridad" de Inicio | Un campo en `config.rs` |

Mientras no se aprueben, las pantallas se diseñan con lo que **ya existe**, y los huecos quedan marcados como `[L#]` en los wireframes.

---

## 2. Design tokens

Todo vive en `ui/tokens.slint` (`global Tokens`). Cero literales en componentes ni pantallas.

### 2.1 Color

Cálculos de contraste WCAG hechos con la fórmula de luminancia relativa. Texto normal ≥ 4,5:1, texto grande y componentes ≥ 3:1.

| Token | Oscuro | Claro | Uso |
|---|---|---|---|
| `bg-base` | `#141517` | `#F4F5F6` | Fondo de la ventana (nunca negro puro) |
| `bg-surface` | `#1A1C1F` | `#FFFFFF` | Hoja de contenido, tarjetas |
| `bg-elevated` | `#212428` | `#FFFFFF` + borde | Filas, inputs, popovers |
| `bg-overlay` | `#272A2F` | `#FFFFFF` | Modales (opaco) |
| `scrim` | `#0B0C0E` @ 60 % | `#1A1D21` @ 40 % | Fondo atenuado tras modales |
| `text-primary` | `#E6E8EB` | `#1A1D21` | Títulos, cifras, cuerpo |
| `text-secondary` | `#A3A9B1` | `#565D66` | Descripciones, etiquetas |
| `text-disabled` | `#5F656D` | `#9AA1A9` | Deshabilitado (exento de AA) |
| `border-subtle` | `#2C3036` | `#E1E4E8` | Divisores, bordes de tarjeta |
| `border-control` | `#6B727B` | `#8A929B` | Contorno de inputs/checkbox/toggle (≥ 3:1) |
| `accent` | `#5CCFC4` | `#0F766E` | Acción principal, estado activo |
| `accent-hover` | `#7ADCD2` | `#115E59` | |
| `accent-active` | `#44B8AE` | `#0D5C56` | |
| `accent-subtle` | `#1B3A38` | `#DDF3F0` | Fondo de ítem activo, badges de acento |
| `on-accent` | `#06211F` | `#FFFFFF` | Texto sobre acento |
| `success` / `-subtle` | `#5FCB8A` / `#173325` | `#17703F` / `#E1F4E9` | Hecho, seguro |
| `warning` / `-subtle` | `#E3B341` / `#3A2F12` | `#8A5B00` / `#FBF0D2` | Revisar |
| `danger` / `-subtle` | `#F0766B` / `#3D1F1D` | `#B42318` / `#FCE6E3` | **Solo amenazas confirmadas** y acciones destructivas en el botón |
| `info` / `-subtle` | `#6FA8E8` / `#17283D` | `#1D5FB8` / `#E3EEFB` | Datos neutros |
| `focus-ring` | `#7ADCD2` | `#0F766E` | Anillo de foco 2 px con 2 px de separación |

Contrastes medidos (texto sobre `bg-base` / `bg-surface` / `bg-elevated`):

| Par | Oscuro | Claro (sobre base / surface) |
|---|---|---|
| `text-primary` | 14,9 / 13,9 / 12,7 | 15,5 / 16,9 |
| `text-secondary` | 7,7 / 7,2 / 6,6 | 6,1 / 6,7 |
| `accent` como texto o ícono | 9,7 / 9,1 / 8,3 | 5,0 / 5,5 |
| `success`, `warning`, `danger`, `info` como texto | todos ≥ 5,6 | todos ≥ 4,6 |
| `on-accent` sobre `accent` | 9,0 (hover 10,4 · active 7,0) | 5,5 (hover 7,6) |

Pendiente de validar con el script de la etapa 1: `border-control` (objetivo ≥ 3:1 sobre su superficie) y los pares `-subtle` + texto del mismo color.

Regla de uso del color: el acento aparece como **máximo en un botón relleno por pantalla** + el ítem activo del menú + progreso. Estados (éxito/aviso) siempre llevan **ícono + texto**, nunca solo color.

### 2.2 Tipografía

- **Inter** (variable, subconjunto latino + español, `tnum` congelado) → UI. **JetBrains Mono** (Regular, subconjunto) → rutas, hashes, comandos.
- Mínimo absoluto 12 px. Pesos usados: 400, 500, 600 (Inter variable permite 500 sin costo).

| Token | Tamaño / interlínea | Peso | Uso |
|---|---|---|---|
| `type-display` | 40 / 44 | 500 | La cifra héroe (ej. "4,2 GB") |
| `type-title` | 24 / 32 | 600 | Título de pantalla |
| `type-heading` | 16 / 24 | 600 | Título de tarjeta/sección |
| `type-body` | 14 / 20 | 400 | Texto general |
| `type-body-strong` | 14 / 20 | 600 | Énfasis, botones |
| `type-caption` | 12 / 16 | 400 | Descripciones, ayudas |
| `type-overline` | 12 / 16 | 600 | Etiquetas de grupo (sin mayúsculas forzadas) |
| `type-mono` | 12 / 16 | 400 | Rutas, hashes |
| `type-metric` | 28 / 32 | 500 | Cifras de tarjetas de métrica |

### 2.3 Espaciado (base 4 px)
`s-1 4 · s-2 8 · s-3 12 · s-4 16 · s-5 20 · s-6 24 · s-8 32 · s-10 40 · s-12 48`.
Reglas: padding de página `s-8` (32); entre bloques `s-6`; dentro de tarjeta `s-4`; entre ícono y texto `s-2`.
Alturas de control: `control-sm 28 · control-md 36 · control-lg 44` (objetivo táctil/clic cómodo). Fila de lista: `row-h 56`, `row-h-compact 40`.

### 2.4 Radios, bordes, elevación
- Radios: `r-sm 4` (checkbox, badge chico) · `r-md 8` (botones, inputs, filas) · `r-lg 12` (tarjetas) · `r-xl 16` (modales) · `r-full`.
- Borde: `1px border-subtle` en tarjetas; `1px border-control` en inputs; 2 px de foco.
- Elevación (sin sombras difusas): 0 `bg-base` → 1 `bg-surface` → 2 `bg-elevated` → 3 `bg-overlay` + `scrim` detrás. Una sola línea de luz superior de 1 px en modales (`border-subtle`) para separarlos.

### 2.5 Motion
| Token | Valor | Uso |
|---|---|---|
| `dur-fast` | 90 ms | hover, press |
| `dur-base` | 160 ms | toggles, cambio de estado, entrada de toast |
| `dur-slow` | 240 ms | apertura de modal, barras de progreso |
| `ease-standard` | `ease-in-out` | cambios de estado |
| `ease-enter` | `ease-out` | entradas |
Prohibido: animar nada que no sea estado o progreso. Con "Reducir animaciones": duraciones 0 y el giro del anillo se reemplaza por un indicador estático con texto.

### 2.6 Iconos
Lucide, trazo `1.5 px` (en caja de 24), redondeado. Tamaños: `icon-sm 16` (en texto y botones), `icon-md 20` (menú, filas), `icon-lg 24` (tarjetas), `icon-xl 40` (estados vacíos). Color: `text-secondary` por defecto, `accent` activo, semánticos solo en badges de estado.

---

## 3. Layout y navegación

### 3.1 Estructura
```
┌──────────┬───────────────────────────────────────────────┐
│ ◉ lumen  │  Título de pantalla                 [acción ▸] │
│          │  una línea que explica                         │
│ Inicio   │ ──────────────────────────────────────────────│
│ Limpieza │                                                │
│ Rendim.  │              contenido (máx. 880 px,           │
│ Programas│               centrado si sobra)               │
│ Segurid. │                                                │
│          │                                                │
│ Historial│                                                │
│ ⚙ Config │                                                │
└──────────┴───────────────────────────────────────────────┘
 216 px  (o 64 px solo íconos si la ventana < 880 px)
```
- Fondo de ventana `bg-base`; el contenido va directo sobre `bg-surface` de borde a borde (sin "hoja flotante" que obliga a redondear sobre transparencia).
- **Tamaño mínimo 760 × 560** (hoy 720 × 640: el alto es más estricto de lo necesario). Tamaño inicial 1000 × 700.
- Al redimensionar: menú pasa a 64 px (solo íconos + tooltip) bajo 880 px; el contenido tiene ancho máximo 880 px; las filas de tarjetas de métricas pasan de 3 columnas a 1 bajo 640 px de contenido.

### 3.2 Navegación propuesta (D2)

| Menú | Pestañas internas | Páginas actuales |
|---|---|---|
| Inicio | — | 0 |
| Limpieza | Limpiar · Duplicados · Espacio | 3 (ya tiene pestañas) |
| Rendimiento | Ajustes · Procesos · Red | 10 · 1 · 12 |
| Programas | — | 7 |
| Seguridad | Revisión · Privacidad | 8 · 11 |
| Mantenimiento | Reparación · Actualizaciones | 13 · 14 |
| Historial | — | 15 |
| Configuración (abajo) | — | 9 |

8 ítems en lugar de 11, agrupados por intención del usuario. Privacidad pasa a Seguridad (es protección) y Red a Rendimiento (es velocidad). **Se puede ajustar**; lo importante es que cambie solo `page` y no la lógica.

### 3.3 Atajos de teclado
| Atajo | Acción |
|---|---|
| `Ctrl+1` … `Ctrl+8` | Ir a cada sección del menú |
| `Ctrl+,` | Configuración |
| `F5` o `Ctrl+R` | Analizar / escanear (la acción de análisis de la pantalla actual) |
| `Ctrl+Enter` | Ejecutar la acción principal (abre la vista previa; **nunca** ejecuta sin confirmar) |
| `Esc` | Cerrar modal / cancelar tarea en curso (con L1) |
| `Tab` / `Shift+Tab`, flechas | Navegar; `Enter`/`Espacio` activan; flechas dentro de listas y menú |
| `Ctrl+Z` | No se usa: deshacer siempre es explícito desde Historial |
Se implementan con un `FocusScope` en la raíz (`key-pressed`) + una línea de ayuda en Configuración.

---

## 4. Rediseño por pantalla

Convención de wireframes: `[ Botón ]` primario, `( Botón )` secundario, `▓▓░░` progreso, `✓` éxito, `[L#]` requiere lógica aún no aprobada.

### 4.1 Inicio
**Objetivo:** "¿Cómo está mi PC y qué hago ahora?" en 3 segundos.

```
Inicio
Tu PC de un vistazo.
┌ Estado ──────────────────────────────────────────────┐
│ ✓ Todo en orden            (o: ● 3 cosas para mejorar)│
│ Podés liberar 4,2 GB y 2 apps arrancan con Windows.   │
│                                         [ Analizar ]   │
└──────────────────────────────────────────────────────┘
┌ Espacio ───┐ ┌ Memoria ───┐ ┌ Procesador ┐ ┌ Seguridad ─┐
│ 212 GB     │ │ 61 %       │ │ 14 %       │ │ Revisada   │
│ libres     │ │ ▓▓▓▓▓░░░   │ │ ▓░░░░░░░   │ │ hace 3 días│
│ ▓▓▓▓▓▓░░   │ │ 9,8/16 GB  │ │ Ryzen 5…   │ │ [L5]       │
└────────────┘ └────────────┘ └────────────┘ └────────────┘
Almacenamiento   C:  ▓▓▓▓▓▓▓░░░  212 GB libres de 476 GB
                 D:  ▓▓░░░░░░░░  …
▸ Detalles del equipo  (colapsado: Sistema, CPU, GPU, placa, red)
```
- **Jerarquía:** (1) tarjeta de estado con **una** acción recomendada; (2) 4 métricas; (3) discos; (4) ficha técnica colapsada (hoy ocupa media pantalla).
- **Presets** (Mínimo / Recomendado / Avanzado) salen del héroe y aparecen *después* de analizar, dentro de la tarjeta de estado como control segmentado + resumen "N ajustes".
- **Estados:** sin analizar (texto "Analizá tu PC para ver qué se puede mejorar" + `[Analizar]`), analizando (anillo de progreso + paso actual + `(Cancelar)` [L1]), listo (presets + `[Optimizar]` → vista previa), aplicado (resumen exacto de lo hecho + `(Ver en Historial)`), error (tarjeta `warning` con "No pudimos medir X" + `(Reintentar)`). Skeleton en las métricas mientras carga el hardware.

### 4.2 Limpieza
**Objetivo:** "Liberar espacio sabiendo exactamente qué se borra".

```
Limpieza            ( Limpiar | Duplicados | Espacio )
4,2 GB  que podés liberar
Todo va a la reserva 7 días: podés deshacerlo.
┌──────────────────────────────────────────────────────┐
│ ☑ Archivos temporales            2,1 GB   ▸          │
│   Archivos que podés borrar sin riesgo               │
│ ☑ Caché del navegador            1,4 GB   ▾          │
│     C:\Users\…\Cache\…            (mono)   312 MB     │
│ ☑ Restos de Windows Update       0,7 GB   ▸          │
└──────────────────────────────────────────────────────┘
Seleccionado: 4,2 GB                  [ Revisar y limpiar ]
```
- Categorías con **casilla + tamaño + detalle expandible** (rutas en mono). Hoy no se puede elegir ni ver el detalle: solo "Limpiar todo". *(Elegir categorías requiere que la lógica acepte un subconjunto: es L6, ver abajo; sin eso, las casillas se muestran marcadas y bloqueadas.)*
- `[Revisar y limpiar]` abre el **modal de impacto** (§5): lista, MB, "va a la reserva 7 días".
- **Después:** resumen con cifra, qué categorías, `(Deshacer)` y `(Borrar ahora)` (la acción irreversible lleva su propia confirmación).
- **Duplicados:** igual patrón; grupos con "Se conserva: ruta (mono) · 2 copias · 380 MB". **Espacio:** barras horizontales por carpeta.
- **Estados:** calculando (skeleton de 4 filas), nada para limpiar (ícono `check` + "Tu disco está limpio"), limpiando (barra con paso), error por archivo en uso (lista de omitidos), éxito.

> **L6 (nuevo):** permitir limpiar un subconjunto de categorías. Hoy `clean-all()` limpia todo. Lo propongo porque el brief pide "agrupado por categoría con vista previa"; sin L6 se muestra el detalle expandible igual (solo lectura).

### 4.3 Rendimiento
**Objetivo:** "Que mi PC ande más rápida, entendiendo cada cambio".
Pestañas: **Ajustes · Procesos · Red**.

```
Rendimiento        ( Ajustes | Procesos | Red )
Cambios seguros para que Windows responda mejor.
Recomendados
┌────────────────────────────────────────────────────┐
│ ☑ Plan de energía de alto rendimiento   ✓ Seguro   │
│   Usa más batería a cambio de más velocidad.        │
│ ☐ Desactivar efectos visuales           ● Revisar  │
└────────────────────────────────────────────────────┘
2 marcadas · Antes de aplicar ves qué se va a cambiar   [ Aplicar ]
```
- **Ajustes** = la plantilla de opciones actual, con badge **Seguro / Revisar** (ícono + texto + tooltip que explica la diferencia) y `[Aplicar]` → modal de vista previa (ya existe).
- **Procesos**: tabla con columnas Nombre · CPU · Memoria · Ubicación (mono). Filas **neutras** al pasar el mouse; la acción es un botón `Cerrar` explícito al final de la fila (visible siempre, ghost) que abre confirmación mostrando "Perdés lo que no hayas guardado". Se elimina el rojo al hover. Búsqueda arriba. Encabezados con flecha de orden.
- **Programas de inicio con impacto alto/medio/bajo y toggle reversible** → **L4** (la pantalla se eliminó; el core conserva `startup.rs`). Queda un hueco diseñado: tarjeta "Al iniciar Windows" con toggles y badge de impacto.
- **Estados:** cargando (skeleton), aplicando (progreso por ajuste), aplicado (`✓ Aplicado` en la fila + enlace a Historial), error por ajuste (fila `warning` con motivo + Reintentar), proceso ya cerrado (toast neutro).

### 4.4 Seguridad
**Objetivo:** "¿Estoy tranquilo? ¿Qué encontró y qué hago?" — sin alarmismo.
Pestañas: **Revisión · Privacidad**.

```
Seguridad          ( Revisión | Privacidad )
┌──────────────────────────────────────────────────────┐
│  (◯ anillo)   Revisar mi PC                           │
│  ( 64% )      Mira Defender, lo que arranca solo,    │
│               conexiones, procesos y drivers.        │
│               Paso 3 de 5: conexiones…   ( Cancelar )[L1]│
│ Tipo: ( Rápida | Completa | Personalizada )     [L3] │
└──────────────────────────────────────────────────────┘
Resultado de la última revisión
  ● 2 cosas para revisar                    (info/warning)
  ┌ Media · Programa nuevo al iniciar ───────────────┐
  │ "updater.exe" empezó a arrancar solo.             │
  │ C:\Users\…\updater.exe                   (mono)   │
  │                    ( Ignorar ) ( Deshabilitar )   │
  └───────────────────────────────────────────────────┘
  ✓ Windows Defender activo · ✓ Firewall activo
Cuarentena (0)  — Nada en cuarentena.              [L3]
```
- **Jerarquía:** anillo + una acción → resultados ordenados por severidad → estado de protección → cuarentena.
- **Lenguaje:** severidad *Alta/Media/Baja* con `warning`/`info`/neutro. **`danger` solo para una detección confirmada de Defender o del motor de firmas.** El texto del resultado vacío mantiene la honestidad actual ("no garantiza que la PC esté limpia").
- Cada hallazgo muestra *qué es, por qué importa, qué pasa si lo deshabilitás* y que es reversible ("Restaurar"). Los comandos decodificados van en mono dentro de un bloque `bg-elevated`.
- **Estados:** sin revisar (invitación + `[Revisar]`), revisando (anillo con % real si L2; si no, anillo indeterminado + paso de texto), limpio, con hallazgos, error (no se pudo consultar Defender: tarjeta `warning` + Reintentar), cancelado [L1].

### 4.5 Historial
**Objetivo:** "Ver qué hizo Lumen y poder volver atrás".

```
Historial                                   🔎 Buscar
Hoy
┌────────────────────────────────────────────────────┐
│ ✓ Limpieza · 4,2 GB liberados          14:32       │
│   Temporales, caché · 1.204 archivos  ( Deshacer ) │
├────────────────────────────────────────────────────┤
│ ✓ Desactivó "updater.exe" al inicio    11:05       │
│   Registro: HKCU\…\Run                    (mono)   │
│                                        ( Deshacer ) │
└────────────────────────────────────────────────────┘
Ayer …
```
- Agrupado por día (si el diario ya guarda fecha; si no, se muestra plano con la hora). Cada fila: ícono por tipo, título, detalle expandible (archivos, MB, procesos, rutas en mono), estado (Aplicado / Deshecho / No se puede deshacer) y `(Deshacer)`.
- **Estados:** vacío (ícono + "Todavía no hiciste ningún cambio con Lumen"), deshaciendo (fila con spinner), deshecho (fila atenuada con tilde), error al deshacer (fila `warning` + motivo).

### 4.6 Configuración
**Objetivo:** "Ajustar la app a mi gusto, sin sorpresas".

```
Configuración
┌ Apariencia ──────────────────────────────────────┐
│ Tema            ( Oscuro | Claro | Automático )   │
│ Reducir animaciones                     [toggle] D4│
│ Idioma          ( Español | English )             │
├ Datos ───────────────────────────────────────────┤
│ Actualizar el panel cada   ( 1 s | 2 s | 5 s )    │
├ Atajos de teclado ───────────────────────────────┤
│ Ctrl+1…8  Secciones    F5  Analizar               │
└──────────────────────────────────────────────────┘
lumen 0.5.0 · Licencia MIT · Iconos Lucide (ISC) · Inter y JetBrains Mono (OFL)
```
- Se guarda sola (como hoy). **Estados:** solo normal (sin carga); error de guardado → toast `warning`.

### 4.7 Programas (no está en tu lista, pero existe)
Lista con ícono, nombre, editor (caption), tamaño y botón `Desinstalar` **siempre visible** (ghost, `danger` solo al hover/foco). Búsqueda arriba; el orden pasa a ser un menú segmentado de una línea. Tras desinstalar: tarjeta "Quedaron N restos" con rutas en mono, `[Mover a la reserva]` / `(Dejarlos)` / `(Deshacer)`. Estados: cargando (skeleton), sin resultados, desinstalando (pasa por el desinstalador del programa), restos, error.

### 4.8 Sección "Privacidad / Red / Reparación / Actualizaciones"
Comparten la **plantilla de opciones** de Rendimiento > Ajustes (un solo componente `OptionList`). Reparación y Actualizaciones muestran además su resultado como tarjeta de estado (éxito/aviso) en vez de una línea de texto suelta.

---

## 5. Librería de componentes

Estados obligatorios en todos: **default · hover · pressed · focus (anillo 2 px) · disabled**. Cada uno con `accessible-role` y `accessible-label`.

| Componente | Variantes / notas |
|---|---|
| **Button** | `primary` (acento relleno, máx. 1 por pantalla), `secondary` (borde `border-control`), `ghost` (solo texto), `danger` (texto/borde rojo, relleno solo en el modal destructivo). Tamaños `sm 28 / md 36 / lg 44`. Con ícono opcional a la izquierda. Foco: anillo. Disabled: opacidad 40 % + sin cursor de mano. |
| **IconButton** | 36×36, ghost, con tooltip obligatorio. |
| **TextField / Search** | Altura 36, borde `border-control`, foco = borde acento + anillo; limpiar con ×; placeholder `text-secondary`. |
| **Toggle** | 36×20, para ajustes con efecto inmediato y reversible (inicio). |
| **Checkbox / Radio** | 20 px, tilde Lucide `check`; para listas de selección + aplicar. |
| **Segmented / Tabs** | Pestañas subrayadas para secciones; segmented para 2–3 opciones cortas. Navegación con flechas. |
| **MetricCard** | Etiqueta (overline) · valor (`type-metric`, tabular) · barra · detalle. Altura fija, 3–4 por fila. |
| **ListRow** | Único componente de fila (reemplaza las 5 copias): ícono · título · descripción · valor a la derecha · acción · chevron de expandir. Alturas 56 / 40. Divisor `border-subtle`. |
| **Table** | Cabeceras ordenables con flecha, cifras alineadas a la derecha, columna mono para rutas, scroll con altura calculada por el propio componente (adiós `content-h` manual). |
| **StatusBadge** | `success / warning / danger / info / neutral`: **ícono + texto**, fondo `-subtle`. Seguro, Revisar, Aplicado, Deshecho, Alta/Media/Baja. |
| **ProgressBar** | Determinada (valor) e indeterminada (tramo que va y viene; estática con "Reducir animaciones"). 4 px, `dur-slow`. |
| **ProgressRing** | Arco de `Path` recalculado; centro con % o ícono; tamaños 48 / 96. Reemplaza `ScanButton`. |
| **Tooltip** | Aparece a los 500 ms de hover o al foco; máx. 240 px; `bg-overlay`. |
| **ConfirmDialog** | Título (qué voy a hacer) · **bloque de impacto** (lista: qué, cuánto, dónde queda, si es reversible) · botones `Cancelar` (foco inicial en acciones destructivas) / acción. `Esc` cancela. |
| **Toast** | Abajo a la derecha, 4 s, tipos `success/info/warning`, con `(Deshacer)` opcional. Máx. 1 visible. Reemplaza los textos sueltos `status`/`*-result`. |
| **Skeleton** | Bloques `bg-elevated` con pulso lento (estático con "Reducir animaciones"). Para métricas, filas y tablas. |
| **EmptyState** | Ícono 40 px · frase corta · acción opcional. |
| **InlineNotice** | Franja con ícono y texto para avisos persistentes (p. ej. "Sin administrador: algunos cambios no se aplicarán"). |
| **Icon** | `Path` generado desde Lucide; tamaños `icon-*`. |
| **Page / PageHeader / Section** | Esqueleto común: título, subtítulo, acción de cabecera, ancho máximo. |

---

## 6. Plan de implementación

### 6.1 Estructura de archivos
```
crates/ui/ui/
  main.slint                 # solo la ventana: layout, navegación, atajos, modales globales
  tokens.slint               # global Tokens (color, tipo, espacio, radio, motion) + tema oscuro/claro
  fonts/                     # Inter-Variable-subset.ttf, JetBrainsMono-Regular-subset.ttf (+ licencias OFL)
  icons.slint                # component Icon (paths Lucide) — generado por script
  components/
    button.slint  field.slint  toggle.slint  check.slint  tabs.slint
    list-row.slint  table.slint  badge.slint  progress.slint  metric-card.slint
    dialog.slint  toast.slint  tooltip.slint  skeleton.slint  empty-state.slint  scroll.slint
  screens/
    home.slint  cleaning.slint  performance.slint  security.slint
    history.slint  settings.slint  programs.slint  option-list.slint
scripts/
  build-icons.py             # Lucide SVG → Path (una vez; el resultado se versiona)
  prepare-fonts.md           # cómo se generó el subconjunto + tnum (reproducible)
```
Las pantallas reciben datos con `in`/`in-out` y `<=>` desde `Main`, así que **`main.rs`, `sections.rs` y `duplicates.rs` no cambian** (salvo los puntos aprobados D1/D3/D4 y L#).

### 6.2 Etapas (cada una cierra con `cargo build`, sin warnings, y te muestro el resultado)

| Etapa | Contenido | Verificación |
|---|---|---|
| **0. Línea base** | Medir exe, RAM y arranque actuales (`check-limits.ps1` con consola de administrador) y guardar capturas de cada pantalla. | Tabla "antes" en este documento |
| **1. Tokens y theme** | `tokens.slint` oscuro + claro; `Theme` actual queda como alias hasta migrar. Script de contraste que valida todos los pares de §2.1. | Build + script de contraste en verde |
| **2. Fuentes e iconos** | Embeber Inter y JetBrains Mono (`import "fonts/…"`), `default-font-family: "Inter"`. Convertir Lucide a `Path`. **Prototipo previo**: confirmar que el eje de peso de la fuente variable funciona en el renderer software. | Ventana de prueba con todos los pesos/íconos; medir ΔMB y ΔRAM |
| **3. Componentes base** | Button, Field, Toggle, Check, Tabs, Badge, Progress*, ListRow, Table, Dialog, Toast, Tooltip, Skeleton, EmptyState, con foco y accesibilidad. | Pantalla temporal "galería" con todos los estados |
| **4. Layout y navegación** | Menú agrupado (D2), colapso a íconos, atajos, `FocusScope` raíz, tamaño mínimo. | Teclado completo recorrible |
| **5. Migración por pantalla** | Orden: Configuración → Historial → Inicio → Limpieza → Rendimiento → Programas → Seguridad. Una pantalla por commit. | Comparación antes/después por pantalla |
| **6. Estados y pulido** | Vacíos, cargando, error y éxito en todas; toasts; microinteracciones; revisión de textos (español simple + `.po`). | Checklist de estados |
| **7. Limpieza** | Borrar `Theme` viejo, componentes huérfanos, camino Mica/`solid`, `docs/DISENO.md` actualizado (ES/EN). | `cargo clippy -D warnings`, `check-limits.ps1` |

### 6.3 Presupuesto de peso y RAM (estimado; se mide en las etapas 0 y 2)
| Concepto | Estimación | Nota |
|---|---|---|
| Inter variable, subconjunto latino | ~300–350 KB | El original completo pesa ~800 KB; se recorta |
| JetBrains Mono Regular, subconjunto | ~80–100 KB | Un solo peso |
| Íconos Lucide como `Path` | < 10 KB | Texto en el binario |
| Nuevos componentes Slint | ~100–200 KB | Compensado al borrar ~25 % de código duplicado |
| **Exe** | **9,3 MB → ~9,8 MB** (límite 15 MB) | Sin dependencias nuevas |
| **RAM** | +1–2 MB posibles (caché de glifos de 2 familias) | Límite 20 MB: si la línea base está cerca, se avisa |
| **Arranque** | Riesgo medio: cargar y analizar dos fuentes al inicio | Límite 300 ms; si se acerca, un solo peso de Mono diferido |

Si alguna medición empeora de forma notable, **te aviso antes de seguir**.

### 6.4 Riesgos y mitigación
| Riesgo | Prob. | Mitigación |
|---|---|---|
| El renderer software no aplica bien los pesos de la fuente variable | Media | Prototipo en etapa 2; plan B: 3 instancias estáticas de Inter (Regular/Medium/SemiBold, ~+450 KB) |
| Congelar `tnum` deja dígitos anchos en prosa | Baja | Aceptable; plan B: segunda familia "Inter Num" solo para cifras (+300 KB) |
| Foco y teclado en Slint son más manuales que en la web | Alta | Se concentra en 4 componentes base y se prueba con la galería de la etapa 3 |
| Pasar de Mica a opaco cambia la identidad visual | Media | Decisión D1 explícita; la etapa 1 deja alternar temas para comparar |
| Reescribir textos rompe las traducciones (`.po` indexa por el texto en español) | Media | Cada cambio de redacción actualiza el `.po` en el mismo commit; script que detecta msgids huérfanos |
| Alturas de scroll calculadas a mano en Rust (`finding_height`, `sc-h`) | Media | El nuevo `Table`/`ListRow` calcula su propio alto; mientras no se migre, se respetan los valores actuales |
| Dividir `main.slint` en módulos con muchas propiedades | Baja | `<=>` desde `Main`; sin cambios en Rust |
| Animación continua (`animation-tick`) gasta CPU en reposo | Baja | Solo mientras `busy` (como hoy); "Reducir animaciones" la apaga |
| Las propuestas L1–L6 no se aprueban | — | Cada pantalla tiene su versión "con lo que existe hoy" |

---

## 7. Próximo paso
Esperando tu aprobación. Necesito respuesta a **D1–D4** (y qué **L1–L6** querés que implemente, aparte). Con D1 y D2 aprobados arranco por las etapas 0 y 1.
