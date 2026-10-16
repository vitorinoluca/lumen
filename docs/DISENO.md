# Diseño de lumen
nEnglish: [DESIGN.md](DESIGN.md)

Para gente común: se entiende sin saber de computadoras. Es una herramienta, no una landing.
Densa donde hace falta (procesos, archivos), despejada en el inicio.

## Tono
- Calma y honestidad: nunca asusta. Dice qué pasa, qué va a hacer y que se puede deshacer.
- Textos cortos y concretos ("Liberá 4,2 GB"), sin marketing.
- Rápida: abre al instante, sin animaciones largas ni bienvenida.

## Reglas visuales
- La ventana es transparente: se ve el material Mica de Windows 11 y el claro/oscuro sigue al tema elegido. Las superficies son capas translúcidas (blanco o negro con poca opacidad), nunca rellenos sólidos. Solo los diálogos son opacos.
- El contenido vive en una "hoja" redondeada junto a un menú sin fondo propio. Los controles no llevan borde; solo líneas finas entre filas de una lista.
- Sin degradés, brillos ni sombras. Sin emojis ni íconos de estrellitas. Íconos de línea, simples.
- Cifras grandes para lo importante (Panel), filas simples para el resto. Tarjetas solo en Panel, Configuración y Seguridad.
- Radios: 6 px controles, 10 px hoja y tarjetas. Alineado a izquierda, grilla de 8 px, márgenes generosos (32 px).
- Una tipografía (Segoe UI Variable). Tamaños: 12 / 14 / 28 (títulos) / 32 (cifras).
- Colores neutros con un solo acento. Rojo solo para algo realmente peligroso.
- Estructura: menú lateral con Inicio, Limpieza, Rendimiento, Privacidad, Red, Reparación, Actualizaciones e Historial, un grupo aparte (Procesos, Programas, Seguridad) y Configuración abajo. Las pantallas con opciones comparten un diseño: casilla, texto corto, etiqueta de riesgo (verde seguro, amarillo revisar), un solo botón principal y una vista previa de lo que va a cambiar.
- Ojo con Slint: no poner `clip` en la hoja translúcida; un recorte redondeado sobre un fondo translúcido rompe el repintado parcial (la zona repintada queda más clara).
- Referencias: Configuración y Archivos de Windows 11, Linear.

## Datos de ejemplo
Reales y desparejos (14,7 GB, 63 °C, 187 procesos), nunca redondos.
