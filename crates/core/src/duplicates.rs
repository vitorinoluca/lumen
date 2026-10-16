//! Buscador de archivos duplicados: idénticos byte a byte. Se compara primero el tamaño, después
//! la huella SHA-256 de los primeros 64 KB y al final la del archivo completo, así casi nunca se lee todo.
//! No borra nada: las copias se mueven a la reserva con `disk::stash_files`.
use sha2::{Digest, Sha256};
use std::{collections::HashMap, env, fs, io::Read, path::{Path, PathBuf}, time::SystemTime};

/// Archivos iguales. El primero es el más antiguo (el original) y es el que se conserva.
#[derive(Debug, Clone)]
pub struct Group {
    pub size: u64,
    pub files: Vec<PathBuf>,
}

impl Group {
    /// Bytes que se liberan al quitar las copias.
    pub fn wasted(&self) -> u64 {
        self.size * (self.files.len() as u64 - 1)
    }
}

/// Carpetas personales donde buscar. Nunca el sistema ni los programas.
pub fn default_roots() -> Vec<PathBuf> {
    let home = PathBuf::from(env::var_os("USERPROFILE").unwrap_or_default());
    ["Downloads", "Documents", "Desktop", "Pictures", "Videos", "Music"].iter().map(|d| home.join(d)).filter(|p| p.is_dir()).collect()
}

/// Carpetas de código y herramientas: miles de archivos repetidos a propósito que no conviene tocar.
fn skipped(name: &str) -> bool {
    matches!(name, "node_modules" | ".git" | "target" | "__pycache__" | ".venv")
}

fn collect(dir: &Path, min: u64, by_size: &mut HashMap<u64, Vec<PathBuf>>) {
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let Ok(t) = e.file_type() else { continue };
        if t.is_dir() && !skipped(&e.file_name().to_string_lossy()) {
            collect(&e.path(), min, by_size);
        } else if t.is_file() {
            // Enlaces simbólicos: no se siguen (t.is_file() es falso para ellos).
            if let Ok(len) = e.metadata().map(|m| m.len()) {
                if len >= min {
                    by_size.entry(len).or_default().push(e.path());
                }
            }
        }
    }
}

/// Huella SHA-256 de los primeros `limit` bytes (o de todo el archivo si es `None`).
fn fingerprint(path: &Path, limit: Option<u64>) -> Option<[u8; 32]> {
    let mut f = fs::File::open(path).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut left = limit.unwrap_or(u64::MAX);
    while left > 0 {
        let want = buf.len().min(left as usize);
        let n = f.read(&mut buf[..want]).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        left -= n as u64;
    }
    Some(h.finalize().into())
}

/// Reparte los archivos por su huella y se queda con los grupos de dos o más.
fn split(files: Vec<PathBuf>, limit: Option<u64>) -> Vec<Vec<PathBuf>> {
    let mut by_hash: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();
    for f in files {
        if let Some(h) = fingerprint(&f, limit) {
            by_hash.entry(h).or_default().push(f);
        }
    }
    by_hash.into_values().filter(|v| v.len() > 1).collect()
}

/// Busca duplicados de al menos `min_size` bytes. Los grupos salen ordenados por espacio que se libera.
pub fn find(roots: &[PathBuf], min_size: u64) -> Vec<Group> {
    let mut by_size = HashMap::new();
    for r in roots {
        collect(r, min_size, &mut by_size);
    }
    let mut groups = vec![];
    for (size, files) in by_size.into_iter().filter(|(_, v)| v.len() > 1) {
        for partial in split(files, Some(64 * 1024)) {
            for mut same in split(partial, None) {
                // El más antiguo es el original.
                same.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH));
                groups.push(Group { size, files: same });
            }
        }
    }
    groups.sort_by_key(|g| std::cmp::Reverse(g.wasted()));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_identical_files_and_ignores_same_size_different_content() {
        let dir = env::temp_dir().join(format!("lumen-dup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::create_dir_all(dir.join("node_modules")).unwrap();
        fs::write(dir.join("a.bin"), vec![7u8; 2000]).unwrap();
        fs::write(dir.join("sub").join("b.bin"), vec![7u8; 2000]).unwrap();
        fs::write(dir.join("c.bin"), vec![8u8; 2000]).unwrap(); // mismo tamaño, otro contenido
        fs::write(dir.join("node_modules").join("d.bin"), vec![7u8; 2000]).unwrap(); // carpeta ignorada
        let groups = find(std::slice::from_ref(&dir), 1000);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        assert_eq!(groups[0].wasted(), 2000);
        assert!(find(std::slice::from_ref(&dir), 5000).is_empty()); // por debajo del mínimo
        let _ = fs::remove_dir_all(&dir);
    }
}
