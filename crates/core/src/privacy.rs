//! Ajustes de privacidad de Windows. Cada uno es un DWORD en el registro;
//! se aplican con `Action::SetDword`, así que se pueden revertir.
use crate::actions::{Action, Hive};
use winreg::{enums::*, RegKey};

pub struct Setting {
    pub name: &'static str,
    pub hive: Hive,
    pub path: &'static str,
    pub value: &'static str,
    /// Valor que deja la función apagada (más privado).
    pub off: u32,
}

const CDM: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";

pub const SETTINGS: &[Setting] = &[
    Setting { name: "ID de publicidad", hive: Hive::User, path: r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo", value: "Enabled", off: 0 },
    Setting { name: "Sugerencias en Inicio y Configuración", hive: Hive::User, path: CDM, value: "SubscribedContent-338388Enabled", off: 0 },
    Setting { name: "Consejos y trucos de Windows", hive: Hive::User, path: CDM, value: "SubscribedContent-338389Enabled", off: 0 },
    Setting { name: "Apps sugeridas en Inicio", hive: Hive::User, path: CDM, value: "SystemPaneSuggestionsEnabled", off: 0 },
    Setting { name: "Experiencias personalizadas con datos de diagnóstico", hive: Hive::User, path: r"Software\Microsoft\Windows\CurrentVersion\Privacy", value: "TailoredExperiencesWithDiagnosticDataEnabled", off: 0 },
    Setting { name: "Búsqueda web de Bing en el menú Inicio", hive: Hive::User, path: r"Software\Policies\Microsoft\Windows\Explorer", value: "DisableSearchBoxSuggestions", off: 1 },
    Setting { name: "Telemetría de Windows (requiere administrador)", hive: Hive::Machine, path: r"SOFTWARE\Policies\Microsoft\Windows\DataCollection", value: "AllowTelemetry", off: 0 },
    Setting { name: "Historial de actividades (requiere administrador)", hive: Hive::Machine, path: r"SOFTWARE\Policies\Microsoft\Windows\System", value: "EnableActivityFeed", off: 0 },
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
