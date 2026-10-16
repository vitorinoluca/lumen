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

#[cfg(test)]
mod tests {
    use super::*;

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
