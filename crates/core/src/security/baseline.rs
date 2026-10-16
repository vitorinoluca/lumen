//! Línea base: lo que había en el primer escaneo y lo que el usuario decidió ignorar.
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, env, fs, io, path::PathBuf};

/// Lo que se guarda de cada entrada de persistencia.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub command: String,
    pub path: String,
    pub sha256: String,
    /// Texto de la firma ("firmado por X", "sin firma"...).
    pub sig: String,
    pub valid: bool,
}

impl Record {
    /// Si cambia el comando o el contenido del archivo, la entrada cuenta como modificada.
    pub fn fingerprint(&self) -> String {
        format!("{}|{}", self.command, self.sha256)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Baseline {
    pub created_secs: u64,
    pub entries: BTreeMap<String, Record>,
    pub exclusions: Vec<String>,
    /// false si la línea base se creó sin poder leer las exclusiones (hace falta administrador).
    #[serde(default)]
    pub exclusions_known: bool,
    pub ignored: BTreeSet<String>,
}

pub fn path() -> PathBuf {
    PathBuf::from(env::var_os("APPDATA").unwrap_or_default()).join("lumen").join("security-baseline.json")
}

impl Baseline {
    pub fn load() -> Option<Self> {
        serde_json::from_slice(&fs::read(path()).ok()?).ok()
    }

    pub fn save(&self) -> io::Result<()> {
        let p = path();
        if let Some(d) = p.parent() {
            fs::create_dir_all(d)?;
        }
        fs::write(p, serde_json::to_vec_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_changes_with_command_or_content() {
        let a = Record { command: "x.exe".into(), sha256: "aa".into(), ..Default::default() };
        let mut b = a.clone();
        assert_eq!(a.fingerprint(), b.fingerprint());
        b.sha256 = "bb".into();
        assert_ne!(a.fingerprint(), b.fingerprint());
        b = a.clone();
        b.command = "y.exe".into();
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn serializes_round_trip() {
        let mut b = Baseline { created_secs: 5, exclusions_known: true, ..Default::default() };
        b.entries.insert("k".into(), Record { command: "c".into(), valid: true, ..Default::default() });
        b.ignored.insert("i".into());
        let back: Baseline = serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();
        assert_eq!((back.created_secs, back.entries.len(), back.ignored.len()), (5, 1, 1));
    }
}
