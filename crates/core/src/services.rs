//! Servicios de Windows. Solo se muestran los de la base curada (`docs/services.toml`),
//! y solo los "safe" y "caution" se pueden cambiar. Los "critical" quedan bloqueados.
//! Desactivar = poner `Start = 4` en el registro (requiere administrador, rige desde el próximo arranque).
use crate::actions::{Action, Hive};
use serde::Deserialize;
use winreg::{enums::*, RegKey};

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Safe,
    Caution,
    Critical,
}

#[derive(Deserialize, Debug)]
struct Entry {
    name: String,
    title: String,
    desc: String,
    level: Level,
}

#[derive(Deserialize)]
struct Db {
    service: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub struct Service {
    pub name: String,
    pub title: String,
    pub desc: String,
    pub level: Level,
    /// 2 = automático, 3 = manual, 4 = desactivado.
    pub start: u32,
}

impl Service {
    pub fn enabled(&self) -> bool {
        self.start != 4
    }
    pub fn locked(&self) -> bool {
        self.level == Level::Critical
    }
}

fn db() -> Vec<Entry> {
    toml::from_str::<Db>(include_str!("../../../docs/services.toml")).expect("services.toml inválido").service
}

fn key(name: &str) -> String {
    format!(r"SYSTEM\CurrentControlSet\Services\{name}")
}

fn start_of(name: &str) -> Option<u32> {
    RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(key(name)).and_then(|k| k.get_value("Start")).ok()
}

/// Servicios de la base que existen en esta PC.
pub fn list() -> Vec<Service> {
    db().into_iter()
        .filter_map(|e| Some(Service { start: start_of(&e.name)?, name: e.name, title: e.title, desc: e.desc, level: e.level }))
        .collect()
}

/// Acción para desactivar un servicio. None si es crítico o no está en la base.
pub fn disable_action(name: &str) -> Option<Action> {
    let e = db().into_iter().find(|e| e.name == name)?;
    (e.level != Level::Critical).then(|| Action::set_dword(Hive::Machine, &key(name), "Start", 4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_is_consistent() {
        let d = db();
        let mut names: Vec<_> = d.iter().map(|e| &e.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), d.len(), "nombres repetidos");
        assert!(d.iter().any(|e| e.level == Level::Critical));
    }

    #[test]
    fn critical_services_cannot_be_disabled() {
        assert!(disable_action("RpcSs").is_none());
        assert!(disable_action("WinDefend").is_none());
        assert!(disable_action("no-existe").is_none());
        match disable_action("DiagTrack") {
            Some(Action::SetDword { new, name, hive, .. }) => assert!(new == 4 && name == "Start" && hive == Hive::Machine),
            _ => panic!("DiagTrack debería poder desactivarse"),
        }
    }

    #[test]
    fn lists_services_from_this_pc() {
        // Servicios básicos que existen en toda instalación de Windows.
        assert!(list().iter().any(|s| s.name == "RpcSs" && s.locked()));
    }
}
