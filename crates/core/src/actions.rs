//! Journal de acciones: toda modificación del sistema pasa por acá y se puede deshacer.
//! Las acciones son datos serializables, así que el deshacer sobrevive a reinicios.
use crate::{power, startup::{self, Source}};
use serde::{Deserialize, Serialize};
use winreg::{enums::*, RegKey};
use std::{fs, io, path::{Path, PathBuf}};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hive {
    User,
    Machine, // requiere administrador
}

impl Hive {
    /// Ruta en el formato que entiende `reg.exe`.
    fn reg_path(self, path: &str) -> String {
        format!(r"{}\{path}", if self == Hive::User { "HKCU" } else { "HKLM" })
    }
    fn root(self) -> RegKey {
        RegKey::predef(if self == Hive::User { HKEY_CURRENT_USER } else { HKEY_LOCAL_MACHINE })
    }
}

/// Acciones reversibles. Cada fase agrega variantes (registro, servicios, ...).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Action {
    /// Mueve un archivo (cuarentena, limpieza). Revert lo devuelve.
    MoveFile { from: PathBuf, to: PathBuf },
    /// Activa o desactiva una app de inicio. Revert aplica el estado contrario.
    /// Archivos movidos a la reserva de limpieza: (original, copia en reserva).
    Stash { items: Vec<(PathBuf, PathBuf)> },
    /// Escribe un DWORD en el registro. `prev` es el valor de antes (None = no existía).
    SetDword { hive: Hive, path: String, name: String, new: u32, prev: Option<u32> },
    /// Borra una clave del registro, guardando antes un respaldo `.reg` para poder restaurarla.
    DeleteRegKey { hive: Hive, path: String, backup: PathBuf },
    /// Cambia el plan de energía. Revert vuelve al plan anterior.
    SetPowerPlan { new: String, prev: String },
    SetStartup { source: Source, name: String, enable: bool },
}

impl Action {
    /// Arma un cambio de plan de energía a partir de un plan base (lo crea si no existe en esta PC).
    pub fn power_plan(template: &str) -> io::Result<Action> {
        Ok(Action::SetPowerPlan { new: power::resolve(template)?, prev: power::active()? })
    }

    /// Arma un SetDword leyendo el valor actual para poder revertirlo.
    pub fn set_dword(hive: Hive, path: &str, name: &str, new: u32) -> Action {
        let prev = hive.root().open_subkey(path).and_then(|k| k.get_value::<u32, _>(name)).ok();
        Action::SetDword { hive, path: path.into(), name: name.into(), new, prev }
    }

    pub fn apply(&self) -> io::Result<()> {
        match self {
            Action::MoveFile { from, to } => mv(from, to),
            Action::Stash { .. } => Ok(()), // se registra ya aplicada, ver `Journal::record`
            Action::SetDword { hive, path, name, new, .. } => write_dword(*hive, path, name, Some(*new)),
            Action::DeleteRegKey { hive, path, backup } => {
                if let Some(d) = backup.parent() {
                    fs::create_dir_all(d)?;
                }
                reg_exe(&["export", &hive.reg_path(path), &backup.to_string_lossy(), "/y"])?;
                hive.root().delete_subkey_all(path)
            }
            Action::SetPowerPlan { new, .. } => power::set(new),
            Action::SetStartup { source, name, enable } => startup::set_enabled(*source, name, *enable),
        }
    }
    pub fn revert(&self) -> io::Result<()> {
        match self {
            Action::MoveFile { from, to } => mv(to, from),
            Action::Stash { items } => {
                // Devuelve lo que se pueda; si algo falla, avisa al final.
                let failed = items.iter().filter(|(orig, saved)| mv(saved, orig).is_err()).count();
                if failed == 0 { Ok(()) } else { Err(io::Error::other(format!("{failed} elementos no se pudieron restaurar"))) }
            }
            Action::SetDword { hive, path, name, prev, .. } => write_dword(*hive, path, name, *prev),
            Action::DeleteRegKey { backup, .. } => reg_exe(&["import", &backup.to_string_lossy()]),
            Action::SetPowerPlan { prev, .. } => power::set(prev),
            Action::SetStartup { source, name, enable } => startup::set_enabled(*source, name, !*enable),
        }
    }
}

fn reg_exe(args: &[&str]) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("reg").args(args).creation_flags(0x0800_0000).output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("reg: {}", String::from_utf8_lossy(&out.stderr).trim())))
    }
}

/// `None` borra el valor (el estado original si no existía).
fn write_dword(hive: Hive, path: &str, name: &str, v: Option<u32>) -> io::Result<()> {
    match v {
        Some(v) => hive.root().create_subkey(path)?.0.set_value(name, &v),
        None => match hive.root().open_subkey_with_flags(path, KEY_SET_VALUE) {
            Ok(k) => match k.delete_value(name) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        },
    }
}

fn mv(from: &Path, to: &Path) -> io::Result<()> {
    if to.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "el destino ya existe"));
    }
    if let Some(p) = to.parent() {
        fs::create_dir_all(p)?;
    }
    fs::rename(from, to)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Entry {
    pub description: String,
    pub action: Action,
    pub applied: bool,
}

/// Registro persistente en JSON (ej. `%APPDATA%\lumen\journal.json`).
pub struct Journal {
    path: PathBuf,
    pub entries: Vec<Entry>,
}

impl Journal {
    pub fn open(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let entries = match fs::read(&path) {
            Ok(b) => serde_json::from_slice(&b)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => vec![],
            Err(e) => return Err(e),
        };
        Ok(Self { path, entries })
    }

    /// Aplica y registra. Si la acción falla no se registra nada. Devuelve el id.
    pub fn apply(&mut self, description: &str, action: Action) -> io::Result<usize> {
        action.apply()?;
        self.entries.push(Entry { description: description.into(), action, applied: true });
        self.save()?;
        Ok(self.entries.len() - 1)
    }

    /// Registra una acción que ya se ejecutó (ej. limpieza parcial).
    pub fn record(&mut self, description: &str, action: Action) -> io::Result<usize> {
        self.entries.push(Entry { description: description.into(), action, applied: true });
        self.save()?;
        Ok(self.entries.len() - 1)
    }

    pub fn revert(&mut self, id: usize) -> io::Result<()> {
        let e = self.entries.get_mut(id).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "id inexistente"))?;
        if !e.applied {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "ya estaba revertida"));
        }
        e.action.revert()?;
        e.applied = false;
        self.save()
    }

    fn save(&self) -> io::Result<()> {
        if let Some(p) = self.path.parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(&self.path, serde_json::to_vec_pretty(&self.entries)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lumen-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn apply_revert_and_persist() {
        let d = tmp("journal");
        let (a, b) = (d.join("a.txt"), d.join("q/a.txt"));
        fs::write(&a, "x").unwrap();
        let jp = d.join("journal.json");

        let mut j = Journal::open(&jp).unwrap();
        let id = j.apply("cuarentena", Action::MoveFile { from: a.clone(), to: b.clone() }).unwrap();
        assert!(!a.exists() && b.exists());

        // reabrir: el deshacer sobrevive al reinicio
        let mut j = Journal::open(&jp).unwrap();
        assert!(j.entries[id].applied);
        j.revert(id).unwrap();
        assert!(a.exists() && !b.exists());
        assert!(j.revert(id).is_err()); // no se revierte dos veces
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn set_dword_restores_previous_state() {
        let path = format!(r"Software\lumen-test-{}", std::process::id());
        let root = Hive::User.root();
        let read = || root.open_subkey(&path).and_then(|k| k.get_value::<u32, _>("v")).ok();
        let tmp = tmp("reg");
        let mut j = Journal::open(tmp.join("j.json")).unwrap();

        let a = j.apply("1", Action::set_dword(Hive::User, &path, "v", 1)).unwrap(); // no existía
        let b = j.apply("2", Action::set_dword(Hive::User, &path, "v", 2)).unwrap(); // prev = 1
        assert_eq!(read(), Some(2));
        j.revert(b).unwrap();
        assert_eq!(read(), Some(1));
        j.revert(a).unwrap();
        assert_eq!(read(), None);
        let _ = root.delete_subkey_all(&path);
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn deleted_reg_key_is_restored_from_backup() {
        let path = format!(r"Software\lumen-test-key-{}", std::process::id());
        let root = Hive::User.root();
        let (k, _) = root.create_subkey(format!(r"{path}\sub")).unwrap();
        k.set_value("v", &7u32).unwrap();
        let tmp = tmp("regkey");
        let mut j = Journal::open(tmp.join("j.json")).unwrap();

        let id = j.apply("borrar", Action::DeleteRegKey { hive: Hive::User, path: path.clone(), backup: tmp.join("b.reg") }).unwrap();
        assert!(root.open_subkey(&path).is_err());
        j.revert(id).unwrap();
        let v: u32 = root.open_subkey(format!(r"{path}\sub")).unwrap().get_value("v").unwrap();
        assert_eq!(v, 7);
        let _ = root.delete_subkey_all(&path);
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn failed_apply_is_not_logged() {
        let d = tmp("fail");
        let mut j = Journal::open(d.join("j.json")).unwrap();
        let r = j.apply("x", Action::MoveFile { from: d.join("nope"), to: d.join("dst") });
        assert!(r.is_err() && j.entries.is_empty());
        let _ = fs::remove_dir_all(d);
    }
}
