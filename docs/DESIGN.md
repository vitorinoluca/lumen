# lumen design

For ordinary people: it must make sense without computer knowledge. A tool, not a landing page.
Dense where needed (processes, files), calm on the home screen.

## Tone
- Calm and honest: never scary. Say what is happening, what will be done, and that it can be undone.
- Short, concrete text ("Free 4.2 GB"), no marketing.
- Fast: opens instantly, no long animations or welcome screens.

## Visual rules
- The window is transparent: Windows 11 Mica shows through, and light/dark follows the theme setting. Surfaces are translucent layers (white or black at low opacity), never solid fills. Only dialogs are opaque.
- Content sits on one rounded "sheet" next to a menu with no background of its own. No borders on controls; hairlines only between list rows.
- No gradients, glow or heavy shadows. No emoji or sparkle icons. Simple line icons.
- Big numbers for what matters (Panel), plain rows for everything else. Cards only on Panel, Settings and Security.
- Radii: 6 px controls, 10 px sheet and cards. Left-aligned, 8 px grid, generous margins (32 px).
- One typeface (Segoe UI Variable). Sizes: 12 / 14 / 28 (titles) / 32 (figures).
- Neutral colors with a single accent. Red only for real danger.
- Layout: side menu with Home, Cleanup, Performance, Privacy, Network, Repair, Updates and History, then a "more" group (Processes, Programs, Security) and Settings at the bottom. Option screens share one layout: checkbox, short text, risk tag (green safe, yellow review), a single main button and a preview of exactly what will change.
- Slint gotcha: no `clip` on the translucent sheet; a rounded clip over a translucent fill breaks partial repaints (the repainted area turns lighter).
- References: Windows 11 Settings and Files, Linear.

## Example data
Real and uneven (14.7 GB, 63 °C, 187 processes), never round.
