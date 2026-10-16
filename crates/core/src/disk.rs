//! Espacio en disco y limpieza reversible.
//! Limpiar mueve los archivos a una reserva (`stash`); el espacio se libera
//! de verdad al vaciarla con `purge`, que ya no se puede deshacer.
use crate::actions::{Action, Journal};
use std::{env, fs, io, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

pub struct Target {
    pub name: &'static str,
    pub path: PathBuf,
}

/// Carpetas de temporales y cachés conocidas que existen en esta PC.
pub fn targets() -> Vec<Target> {
    let var = |k: &str| env::var_os(k).map(PathBuf::from);
    let local = var("LOCALAPPDATA");
    let mut v = vec![];
    if let Some(t) = var("TEMP") {
        v.push(Target { name: "Temporales del usuario", path: t });
    }
    if let Some(w) = var("WINDIR") {
        v.push(Target { name: "Temporales de Windows", path: w.join("Temp") });
    }
    if let Some(l) = local {
        v.push(Target { name: "Caché de Chrome", path: l.join(r"Google\Chrome\User Data\Default\Cache") });
        v.push(Target { name: "Caché de Edge", path: l.join(r"Microsoft\Edge\User Data\Default\Cache") });
    }
    v.retain(|t| t.path.is_dir());
    v
}

/// Tamaño total en bytes (ignora lo que no se puede leer).
pub fn dir_size(p: &Path) -> u64 {
    let Ok(rd) = fs::read_dir(p) else { return 0 };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0, // enlaces simbólicos: no se siguen
        })
        .sum()
}

/// Subcarpetas directas de `root` ordenadas por tamaño, de mayor a menor.
pub fn space_map(root: &Path) -> Vec<(PathBuf, u64)> {
    let mut v: Vec<_> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| (e.path(), dir_size(&e.path())))
        .collect();
    v.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
    v
}

pub fn stash_root() -> PathBuf {
    PathBuf::from(env::var_os("APPDATA").unwrap_or_default()).join("lumen").join("stash")
}

/// Mueve el contenido de `dir` a la reserva y lo registra en el journal.
/// Lo que está en uso se omite. Devuelve (movidos, omitidos).
pub fn clean(journal: &mut Journal, label: &str, dir: &Path, stash: &Path) -> io::Result<(usize, usize)> {
    let batch = stash.join(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis()).to_string());
    fs::create_dir_all(&batch)?;
    let (mut moved, mut skipped) = (vec![], 0);
    for (i, e) in fs::read_dir(dir)?.flatten().enumerate() {
        let to = batch.join(format!("{i}_{}", e.file_name().to_string_lossy()));
        match fs::rename(e.path(), &to) {
            Ok(()) => moved.push((e.path(), to)),
            Err(_) => skipped += 1,
        }
    }
    let n = moved.len();
    if n > 0 {
        journal.record(&format!("Limpiar {label}"), Action::Stash { items: moved })?;
    } else {
        let _ = fs::remove_dir(&batch);
    }
    Ok((n, skipped))
}

/// Borra definitivamente la reserva. NO se puede deshacer. Devuelve los bytes liberados.
pub fn purge(stash: &Path) -> u64 {
    let n = dir_size(stash);
    let _ = fs::remove_dir_all(stash);
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_revert_and_purge() {
        let base = env::temp_dir().join(format!("lumen-disk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (dir, stash) = (base.join("tmp"), base.join("stash"));
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.tmp"), vec![0; 100]).unwrap();
        fs::write(dir.join("sub/b.tmp"), vec![0; 50]).unwrap();
        assert_eq!(dir_size(&dir), 150);
        assert_eq!(space_map(&dir)[0].1, 50);

        let mut j = Journal::open(base.join("j.json")).unwrap();
        let (moved, skipped) = clean(&mut j, "test", &dir, &stash).unwrap();
        assert_eq!((moved, skipped), (2, 0));
        assert_eq!(dir_size(&dir), 0);

        j.revert(0).unwrap();
        assert_eq!(dir_size(&dir), 150);

        clean(&mut j, "test", &dir, &stash).unwrap();
        assert_eq!(purge(&stash), 150);
        assert!(j.revert(1).is_err()); // purgado: ya no hay vuelta atrás
        let _ = fs::remove_dir_all(base);
    }
}
