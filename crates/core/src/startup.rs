//! Apps de inicio: claves Run (usuario y equipo) y carpeta Startup del usuario.
//! Activar/desactivar usa `StartupApproved`, igual que el Administrador de tareas:
//! no borra nada, solo marca la entrada.
use serde::{Deserialize, Serialize};
use std::{env, fs, io, path::PathBuf};
use winreg::{enums::*, RegKey, HKEY};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    RunUser,
    RunMachine,
    FolderUser,
}

impl Source {
    fn hive(self) -> HKEY {
        if self == Source::RunMachine { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER }
    }
    fn approved_key(self) -> String {
        let sub = if self == Source::FolderUser { "StartupFolder" } else { "Run" };
        format!(r"{APPROVED}\{sub}")
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub source: Source,
    pub name: String,
    pub command: String,
    pub enabled: bool,
}

fn folder() -> Option<PathBuf> {
    env::var_os("APPDATA").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs\Startup"))
}

/// Sin valor en StartupApproved = habilitada. Primer byte impar (3) = deshabilitada.
fn is_enabled(source: Source, name: &str) -> bool {
    RegKey::predef(source.hive())
        .open_subkey(source.approved_key())
        .and_then(|k| k.get_raw_value(name))
        .map(|v| v.bytes.first().is_none_or(|b| b % 2 == 0))
        .unwrap_or(true)
}

pub fn list() -> Vec<Entry> {
    let mut out = vec![];
    for source in [Source::RunUser, Source::RunMachine] {
        if let Ok(k) = RegKey::predef(source.hive()).open_subkey(RUN) {
            for (name, _) in k.enum_values().flatten() {
                let command = k.get_value::<String, _>(&name).unwrap_or_default();
                out.push(Entry { source, enabled: is_enabled(source, &name), name, command });
            }
        }
    }
    if let Some(dir) = folder() {
        for f in fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = f.file_name().to_string_lossy().into_owned();
            if name.eq_ignore_ascii_case("desktop.ini") {
                continue;
            }
            out.push(Entry {
                source: Source::FolderUser,
                enabled: is_enabled(Source::FolderUser, &name),
                command: f.path().display().to_string(),
                name,
            });
        }
    }
    out
}

pub fn set_enabled(source: Source, name: &str, enable: bool) -> io::Result<()> {
    let (key, _) = RegKey::predef(source.hive()).create_subkey(source.approved_key())?;
    let mut bytes = vec![0u8; 12];
    bytes[0] = if enable { 2 } else { 3 };
    key.set_raw_value(name, &winreg::RegValue { bytes: bytes.into(), vtype: REG_BINARY })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Action;

    #[test]
    fn disable_and_revert_via_action() {
        let name = format!("lumen-test-{}", std::process::id());
        let (run, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN).unwrap();
        run.set_value(&name, &"notepad.exe").unwrap();
        let find = || list().into_iter().find(|e| e.name == name).unwrap();
        assert!(find().enabled);

        let a = Action::SetStartup { source: Source::RunUser, name: name.clone(), enable: false };
        a.apply().unwrap();
        assert!(!find().enabled);
        a.revert().unwrap();
        assert!(find().enabled);

        run.delete_value(&name).unwrap();
        let approved = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(Source::RunUser.approved_key(), KEY_WRITE).unwrap();
        let _ = approved.delete_value(&name);
    }
}
