//! Journal de acciones: toda modificación del sistema pasa por acá y se puede deshacer.
//! Las acciones son datos serializables, así que el deshacer sobrevive a reinicios.
use serde::{Deserialize, Serialize};
use std::{fs, io, path::{Path, PathBuf}};

/// Acciones reversibles. Cada fase agrega variantes (registro, servicios, ...).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Action {
    /// Mueve un archivo (cuarentena, limpieza). Revert lo devuelve.
    MoveFile { from: PathBuf, to: PathBuf },
}

impl Action {
    pub fn apply(&self) -> io::Result<()> {
        match self {
            Action::MoveFile { from, to } => mv(from, to),
        }
    }
    pub fn revert(&self) -> io::Result<()> {
        match self {
            Action::MoveFile { from, to } => mv(to, from),
        }
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
    fn failed_apply_is_not_logged() {
        let d = tmp("fail");
        let mut j = Journal::open(d.join("j.json")).unwrap();
        let r = j.apply("x", Action::MoveFile { from: d.join("nope"), to: d.join("dst") });
        assert!(r.is_err() && j.entries.is_empty());
        let _ = fs::remove_dir_all(d);
    }
}
