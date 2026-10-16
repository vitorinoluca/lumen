//! Ajustes de privacidad de Windows. Cada uno es un DWORD en el registro;
//! se aplican con `Action::SetDword`, así que se pueden revertir.
use crate::{
    actions::{Action, Hive},
    plan::{Item, Risk, Step},
};
use winreg::{enums::*, RegKey};

pub struct Setting {
    pub name: &'static str,
    /// Qué hace, en una frase simple.
    pub desc: &'static str,
    pub hive: Hive,
    pub path: &'static str,
    pub value: &'static str,
    /// Valor que deja la función apagada (más privado).
    pub off: u32,
}

const CDM: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";

pub const SETTINGS: &[Setting] = &[
    Setting { name: "Desactivar el ID de publicidad", desc: "Evita que las apps te sigan con un identificador para mostrarte anuncios.", hive: Hive::User, path: r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo", value: "Enabled", off: 0 },
    Setting { name: "Desactivar las sugerencias en Inicio y Configuración", desc: "Quita las sugerencias y promociones que Windows muestra en Inicio y Configuración.", hive: Hive::User, path: CDM, value: "SubscribedContent-338388Enabled", off: 0 },
    Setting { name: "Desactivar los consejos y trucos de Windows", desc: "Deja de mostrar notificaciones con consejos y novedades.", hive: Hive::User, path: CDM, value: "SubscribedContent-338389Enabled", off: 0 },
    Setting { name: "Desactivar las apps sugeridas en Inicio", desc: "Quita las apps promocionadas del menú Inicio.", hive: Hive::User, path: CDM, value: "SystemPaneSuggestionsEnabled", off: 0 },
    Setting { name: "Desactivar la personalización con datos de diagnóstico", desc: "Windows no usa tus datos de diagnóstico para personalizar anuncios y consejos.", hive: Hive::User, path: r"Software\Microsoft\Windows\CurrentVersion\Privacy", value: "TailoredExperiencesWithDiagnosticDataEnabled", off: 0 },
    Setting { name: "Desactivar la búsqueda web de Bing en Inicio", desc: "El buscador de Inicio busca solo en tu PC, sin enviar lo que escribís a Bing.", hive: Hive::User, path: r"Software\Policies\Microsoft\Windows\Explorer", value: "DisableSearchBoxSuggestions", off: 1 },
    Setting { name: "Desactivar la telemetría de Windows", desc: "Reduce los datos de uso y diagnóstico que Windows envía a Microsoft.", hive: Hive::Machine, path: r"SOFTWARE\Policies\Microsoft\Windows\DataCollection", value: "AllowTelemetry", off: 0 },
    Setting { name: "Desactivar el historial de actividades", desc: "Windows deja de guardar un historial de lo que hacés en la PC.", hive: Hive::Machine, path: r"SOFTWARE\Policies\Microsoft\Windows\System", value: "EnableActivityFeed", off: 0 },
];

impl Setting {
    pub fn current(&self) -> Option<u32> {
        let root = RegKey::predef(if self.hive == Hive::User { HKEY_CURRENT_USER } else { HKEY_LOCAL_MACHINE });
        root.open_subkey(self.path).and_then(|k| k.get_value(self.value)).ok()
    }

    /// Apagada = el valor ya es el privado. Sin valor, Windows usa su predeterminado (encendida).
    pub fn is_off(&self) -> bool {
        self.current() == Some(self.off)
    }

    pub fn turn_off(&self) -> Action {
        Action::set_dword(self.hive, self.path, self.value, self.off)
    }
}

/// Los ajustes como opciones de la sección Privacidad. Son todos seguros: se revierten con el valor de antes.
pub fn items() -> Vec<Item> {
    SETTINGS
        .iter()
        .map(|s| {
            let mut item = Item::new(s.name, s.name, s.desc, Risk::Safe, vec![Step { action: s.turn_off() }]);
            item.done = s.is_off();
            item
        })
        .collect()
}

/// Apps preinstaladas que casi nadie usa: (paquete, nombre, qué es).
const BLOAT: &[(&str, &str, &str)] = &[
    ("Microsoft.BingNews", "Noticias", "Noticias de Microsoft con publicidad."),
    ("Microsoft.BingWeather", "El Tiempo", "App del clima de Microsoft."),
    ("Microsoft.GetHelp", "Obtener ayuda", "Chat de soporte de Microsoft."),
    ("Microsoft.Getstarted", "Consejos", "Consejos y novedades de Windows."),
    ("Microsoft.MicrosoftSolitaireCollection", "Solitario", "Juegos de cartas con anuncios."),
    ("Microsoft.MixedReality.Portal", "Realidad mixta", "Solo sirve con visores de realidad virtual."),
    ("Microsoft.People", "Contactos", "Libreta de contactos de Microsoft."),
    ("Microsoft.SkypeApp", "Skype", "Videollamadas de Skype."),
    ("Microsoft.WindowsFeedbackHub", "Centro de opiniones", "Envía opiniones y errores a Microsoft."),
    ("Microsoft.ZuneVideo", "Películas y TV", "Reproductor de video de Microsoft."),
    ("Clipchamp.Clipchamp", "Clipchamp", "Editor de video de Microsoft."),
    ("Microsoft.MicrosoftOfficeHub", "Microsoft 365", "Acceso directo a Office con ofertas."),
    ("Microsoft.Todos", "Microsoft To Do", "Lista de tareas de Microsoft."),
    ("Microsoft.YourPhone", "Vincular con el móvil", "Conecta el celular con la PC."),
];

/// Paquetes que se ofrecen quitar (para comprobar textos y traducciones).
pub const BLOAT_PACKAGES: [&str; BLOAT.len()] = {
    let mut out = [""; BLOAT.len()];
    let mut i = 0;
    while i < BLOAT.len() {
        out[i] = BLOAT[i].0;
        i += 1;
    }
    out
};

/// Nombres de los paquetes Appx instalados para este usuario (tarda un par de segundos).
pub fn installed_apps() -> Vec<String> {
    crate::ps::run("Get-AppxPackage | Select-Object -ExpandProperty Name").map(|t| t.lines().map(|l| l.trim().to_string()).collect()).unwrap_or_default()
}

/// Opciones para quitar las apps preinstaladas que estén instaladas. Son de riesgo amarillo,
/// vienen desmarcadas y no se pueden deshacer; van detrás de "Más opciones".
pub fn bloat_items(installed: &[String]) -> Vec<Item> {
    let mut items: Vec<Item> = BLOAT
        .iter()
        .filter(|(pkg, ..)| installed.iter().any(|i| i == pkg))
        .map(|(pkg, name, desc)| {
            let mut it = Item::new(&format!("app:{pkg}"), &format!("Quitar {name}"), &format!("{desc} No se puede deshacer desde lumen."), Risk::Review, vec![Step { action: Action::RemoveApp { package: (*pkg).into() } }]);
            it.more = true;
            it
        })
        .collect();
    if !items.is_empty() {
        items.insert(0, Item { more: true, ..Item::header("Apps preinstaladas") });
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bloat_items_only_list_installed_apps_and_start_unchecked() {
        let items = bloat_items(&["Microsoft.BingNews".to_string(), "Otra.App".to_string()]);
        assert_eq!(items.len(), 2); // el título y una app
        assert!(items.iter().all(|i| i.more && !i.default_checked()));
        assert!(!items[1].steps[0].action.undoable());
        assert!(bloat_items(&[]).is_empty());
    }

    #[test]
    fn turn_off_action_targets_the_private_value() {
        for s in SETTINGS {
            match s.turn_off() {
                Action::SetDword { new, name, .. } => assert!(new == s.off && name == s.value),
                _ => panic!("debe ser SetDword"),
            }
        }
    }
}
