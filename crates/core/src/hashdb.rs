//! Capa 2 de la revisión de seguridad: hashes conocidos.
//! Fuente: LOLDrivers (https://www.loldrivers.io, licencia Apache-2.0, sin registro ni clave): drivers
//! vulnerables o maliciosos. Se descarga solo cuando el usuario lo pide y se guardan únicamente los
//! hashes en `%APPDATA%\lumen\db`. Todo el análisis es local.
//! (abuse.ch MalwareBazaar se descartó: exige una Auth-Key y funciona bajo "uso justo".)
use crate::{
    ps,
    security::{collect::target_of, signature, Finding, Fix, Severity},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs, io, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use winreg::{enums::*, RegKey};

pub const SOURCE_URL: &str = "https://www.loldrivers.io/api/drivers.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    pub name: String,
    pub malicious: bool,
}

#[derive(Debug, Default)]
pub struct Db {
    entries: HashMap<String, Info>,
    pub updated_secs: u64,
}

pub fn dir() -> PathBuf {
    PathBuf::from(env::var_os("APPDATA").unwrap_or_default()).join("lumen").join("db")
}

fn db_file() -> PathBuf {
    dir().join("loldrivers.tsv")
}

#[derive(Deserialize)]
struct RawEntry {
    #[serde(rename = "Category", default)]
    category: Option<String>,
    #[serde(rename = "KnownVulnerableSamples", default)]
    samples: Vec<RawSample>,
}

#[derive(Deserialize)]
struct RawSample {
    #[serde(rename = "Filename", default)]
    filename: Option<String>,
    #[serde(rename = "SHA256", default)]
    sha256: Option<String>,
}

/// (sha256 en minúsculas, info) de cada muestra del JSON de LOLDrivers.
pub fn parse_source(json: &[u8]) -> Result<Vec<(String, Info)>, serde_json::Error> {
    let raw: Vec<RawEntry> = serde_json::from_slice(json)?;
    Ok(raw
        .into_iter()
        .flat_map(|e| {
            let malicious = e.category.as_deref() == Some("malicious");
            e.samples.into_iter().filter_map(move |s| {
                let sha = s.sha256?.trim().to_lowercase();
                (sha.len() == 64).then(|| (sha, Info { name: s.filename.unwrap_or_default(), malicious }))
            })
        })
        .collect())
}

impl Db {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn lookup(&self, sha256: &str) -> Option<&Info> {
        self.entries.get(&sha256.to_lowercase())
    }

    pub fn from_entries(items: Vec<(String, Info)>, updated_secs: u64) -> Db {
        Db { entries: items.into_iter().collect(), updated_secs }
    }

    /// Formato compacto: una línea de cabecera y `sha256 <tab> m|v <tab> archivo` por hash.
    pub fn to_tsv(&self) -> String {
        let mut out = format!("#lumen-hashdb loldrivers {} {}\n", self.updated_secs, self.len());
        let mut rows: Vec<_> = self.entries.iter().collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        for (sha, i) in rows {
            out.push_str(&format!("{sha}\t{}\t{}\n", if i.malicious { 'm' } else { 'v' }, i.name.replace(['\t', '\n'], " ")));
        }
        out
    }

    pub fn from_tsv(text: &str) -> Option<Db> {
        let mut lines = text.lines();
        let head = lines.next()?.strip_prefix("#lumen-hashdb loldrivers ")?;
        let updated_secs = head.split_whitespace().next()?.parse().ok()?;
        let entries = lines
            .filter_map(|l| {
                let mut p = l.splitn(3, '\t');
                let (sha, kind, name) = (p.next()?, p.next()?, p.next().unwrap_or(""));
                (sha.len() == 64).then(|| (sha.to_string(), Info { name: name.to_string(), malicious: kind == "m" }))
            })
            .collect();
        Some(Db { entries, updated_secs })
    }

    pub fn load() -> Option<Db> {
        Db::from_tsv(&fs::read_to_string(db_file()).ok()?)
    }

    fn save(&self) -> io::Result<()> {
        fs::create_dir_all(dir())?;
        fs::write(db_file(), self.to_tsv())
    }
}

/// Descarga la base de LOLDrivers y guarda solo los hashes. Es lo único de lumen que usa internet,
/// y solo cuando el usuario lo pide. Devuelve cuántos hashes quedaron. Usa unos 40 MB mientras procesa.
pub fn update() -> io::Result<usize> {
    let tmp = env::temp_dir().join(format!("lumen-loldrivers-{}.json", std::process::id()));
    let script = format!(
        "$ProgressPreference = 'SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri {} -OutFile {} -TimeoutSec 120",
        ps::quote(SOURCE_URL),
        ps::quote(&tmp.to_string_lossy())
    );
    let downloaded = ps::run(&script);
    let result = downloaded.and_then(|_| fs::read(&tmp)).and_then(|bytes| parse_source(&bytes).map_err(io::Error::other));
    let _ = fs::remove_file(&tmp);
    let items = result?;
    if items.is_empty() {
        return Err(io::Error::other("la descarga no trajo hashes"));
    }
    let db = Db::from_entries(items, SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()));
    db.save()?;
    Ok(db.len())
}

/// La lista se renueva sola, pero no más de una vez por semana: es chica (unos 160 KB) y cambia poco.
const MAX_AGE_SECS: u64 = 7 * 24 * 3600;

/// Actualiza la lista si falta o tiene más de una semana. Ok(true) si se descargó una nueva.
/// Se llama al revisar la PC: es el pedido del usuario, no corre en segundo plano.
pub fn ensure_fresh() -> io::Result<bool> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if Db::load().is_some_and(|d| now.saturating_sub(d.updated_secs) < MAX_AGE_SECS) {
        return Ok(false);
    }
    update().map(|_| true)
}

// ------------------------------------------------------------- caché de hashes

#[derive(Serialize, Deserialize, Default)]
pub struct Cache {
    /// ruta -> (tamaño, fecha de modificación, sha256)
    files: HashMap<String, (u64, u64, String)>,
    #[serde(skip)]
    dirty: bool,
}

fn cache_file() -> PathBuf {
    dir().join("hash-cache.json")
}

impl Cache {
    pub fn load() -> Cache {
        fs::read(cache_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        if self.dirty {
            let _ = fs::create_dir_all(dir());
            let _ = fs::write(cache_file(), serde_json::to_vec(&self.files).unwrap_or_default());
        }
    }

    /// SHA-256 del archivo; si no cambió (misma ruta, tamaño y fecha) no se recalcula.
    pub fn sha256(&mut self, path: &Path) -> Option<String> {
        let meta = fs::metadata(path).ok()?;
        let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let key = path.to_string_lossy().to_lowercase();
        if let Some((size, m, sha)) = self.files.get(&key) {
            if *size == meta.len() && *m == mtime {
                return Some(sha.clone());
            }
        }
        let sha = signature::sha256(path)?;
        self.files.insert(key, (meta.len(), mtime, sha.clone()));
        self.dirty = true;
        Some(sha)
    }
}

// ------------------------------------------------------------- drivers instalados

#[derive(Debug, Clone)]
pub struct DriverFile {
    pub path: PathBuf,
    /// Nombre del servicio y su tipo de inicio, si el driver está registrado.
    pub service: Option<(String, u32)>,
}

fn windir() -> PathBuf {
    PathBuf::from(env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
}

/// Archivos de drivers: los servicios de tipo controlador y todo lo que hay en System32\drivers.
pub fn driver_files() -> Vec<DriverFile> {
    let mut out: Vec<DriverFile> = vec![];
    if let Ok(base) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SYSTEM\CurrentControlSet\Services") {
        for name in base.enum_keys().flatten() {
            let Ok(k) = base.open_subkey(&name) else { continue };
            let (ty, start): (u32, u32) = (k.get_value("Type").unwrap_or(0), k.get_value("Start").unwrap_or(4));
            let image: String = k.get_value("ImagePath").unwrap_or_default();
            if ty & 0x0B == 0 || image.is_empty() {
                continue; // no es un controlador (1 kernel, 2 sistema de archivos, 8 reconocedor)
            }
            if let Some(path) = target_of(&image).filter(|p| p.is_file()) {
                out.push(DriverFile { path, service: Some((name, start)) });
            }
        }
    }
    for e in fs::read_dir(windir().join(r"System32\drivers")).into_iter().flatten().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("sys")) && !out.iter().any(|d| d.path.to_string_lossy().eq_ignore_ascii_case(&p.to_string_lossy())) {
            out.push(DriverFile { path: p, service: None });
        }
    }
    out
}

/// Compara los drivers con la base. Pura respecto del sistema salvo por leer los archivos.
pub fn check_files(db: &Db, files: &[DriverFile], cache: &mut Cache) -> Vec<Finding> {
    let sys_drivers = windir().join(r"System32\drivers").to_string_lossy().to_lowercase();
    let mut out: Vec<Finding> = vec![];
    for f in files {
        let Some(sha) = cache.sha256(&f.path) else { continue };
        let Some(info) = db.lookup(&sha) else { continue };
        let key = format!("hash:{sha}");
        if out.iter().any(|x| x.key == key) {
            continue;
        }
        let file = f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let (severity, kind, why) = if info.malicious {
            (Severity::High, "malicioso", "LOLDrivers lo cataloga como malicioso: se usa para dañar o esconder malware.")
        } else {
            (Severity::Medium, "vulnerable", "Es un driver legítimo con una vulnerabilidad conocida que se puede abusar para tomar control del sistema. Si no usás el programa que lo instaló, desinstalalo; si lo usás, buscá una versión más nueva.")
        };
        // Solo se ofrece deshabilitar drivers de terceros con inicio automático o manual: apagar uno del sistema puede impedir el arranque.
        let third_party = !f.path.to_string_lossy().to_lowercase().starts_with(&sys_drivers);
        let fix = f.service.as_ref().filter(|(_, start)| (2..=3).contains(start) && third_party).map(|(name, _)| Fix::Service { name: name.clone() });
        out.push(Finding {
            key,
            severity,
            title: format!("Driver {kind} conocido: {file}"),
            detail: format!("{}\nFigura en la lista pública de drivers peligrosos (LOLDrivers) como «{}».\n{why}", f.path.display(), info.name),
            decoded: None,
            fix,
        });
    }
    out
}

/// Revisa los drivers instalados contra la lista. Devuelve los hallazgos y un aviso en lenguaje simple.
pub fn check() -> (Vec<Finding>, String) {
    let Some(db) = Db::load() else {
        return (vec![], "No pude revisar los drivers: hace falta conexión a internet una vez para bajar la lista de drivers peligrosos conocidos.".into());
    };
    let files = driver_files();
    let mut cache = Cache::load();
    let findings = check_files(&db, &files, &mut cache);
    cache.save();
    (findings, format!("Revisé {} drivers contra la lista de drivers peligrosos conocidos.", files.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn parses_loldrivers_json_including_nulls() {
        let json = format!(
            r#"[{{"Category":"malicious","KnownVulnerableSamples":[{{"Filename":"a.sys","SHA256":"{SHA_ABC}"}},{{"Filename":"sin-hash.sys","SHA256":null}}]}},
                {{"Category":"vulnerable driver","KnownVulnerableSamples":[{{"Filename":"b.sys","SHA256":"{}"}}]}},
                {{"KnownVulnerableSamples":[]}}]"#,
            "AB".repeat(32)
        );
        let items = parse_source(json.as_bytes()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], (SHA_ABC.to_string(), Info { name: "a.sys".into(), malicious: true }));
        assert!(!items[1].1.malicious && items[1].0 == "ab".repeat(32)); // se normaliza a minúsculas
        assert!(parse_source(b"no es json").is_err());
    }

    #[test]
    fn tsv_round_trip_and_case_insensitive_lookup() {
        let db = Db::from_entries(vec![(SHA_ABC.into(), Info { name: "x\ty.sys".into(), malicious: false })], 42);
        let back = Db::from_tsv(&db.to_tsv()).unwrap();
        assert_eq!((back.len(), back.updated_secs), (1, 42));
        assert_eq!(back.lookup(&SHA_ABC.to_uppercase()).map(|i| i.malicious), Some(false));
        assert!(back.lookup("00").is_none());
        assert!(Db::from_tsv("basura").is_none());
    }

    fn temp(name: &str) -> PathBuf {
        let d = env::temp_dir().join(format!("lumen-hashdb-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn cache_recomputes_only_when_the_file_changes() {
        let d = temp("cache");
        let f = d.join("a.sys");
        fs::write(&f, "abc").unwrap();
        let mut c = Cache::default();
        assert_eq!(c.sha256(&f).as_deref(), Some(SHA_ABC));
        // el valor guardado se usa mientras tamaño y fecha coincidan: lo comprobamos alterando el guardado
        let key = f.to_string_lossy().to_lowercase();
        c.files.get_mut(&key).unwrap().2 = "guardado".into();
        assert_eq!(c.sha256(&f).as_deref(), Some("guardado"));
        // cambia el contenido (y el tamaño): se vuelve a calcular
        fs::write(&f, "abcd").unwrap();
        assert_ne!(c.sha256(&f).as_deref(), Some("guardado"));
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn flags_known_drivers_with_the_right_severity_and_fix_policy() {
        let d = temp("check");
        let (vuln, mal, ok) = (d.join("vuln.sys"), d.join("mal.sys"), d.join("limpio.sys"));
        fs::write(&vuln, "abc").unwrap();
        fs::write(&mal, "otro contenido").unwrap();
        fs::write(&ok, "nada raro").unwrap();
        let mut c = Cache::default();
        let sha_mal = c.sha256(&mal).unwrap();
        let db = Db::from_entries(
            vec![(SHA_ABC.into(), Info { name: "RTCore64.sys".into(), malicious: false }), (sha_mal, Info { name: "evil.sys".into(), malicious: true })],
            1,
        );
        let files = vec![
            DriverFile { path: vuln.clone(), service: Some(("Vuln".into(), 3)) },
            DriverFile { path: mal, service: Some(("Mal".into(), 0)) }, // de arranque: no se ofrece apagarlo
            DriverFile { path: ok, service: None },
        ];
        let f = check_files(&db, &files, &mut c);
        assert_eq!(f.len(), 2);
        let v = f.iter().find(|x| x.title.contains("vuln.sys")).unwrap();
        assert_eq!(v.severity, Severity::Medium);
        assert_eq!(v.fix, Some(Fix::Service { name: "Vuln".into() }));
        let m = f.iter().find(|x| x.title.contains("mal.sys")).unwrap();
        assert_eq!(m.severity, Severity::High);
        assert!(m.fix.is_none(), "un driver de arranque no se puede apagar desde acá");
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn fresh_database_is_not_downloaded_again() {
        // sin conexión ni archivos: si la lista está al día no se toca la red
        let d = temp("fresh");
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let db = Db::from_entries(vec![(SHA_ABC.into(), Info { name: "x".into(), malicious: false })], now);
        let back = Db::from_tsv(&db.to_tsv()).unwrap();
        assert!(now.saturating_sub(back.updated_secs) < MAX_AGE_SECS);
        let old = Db::from_tsv(&Db::from_entries(vec![], now - MAX_AGE_SECS - 10).to_tsv()).unwrap();
        assert!(now.saturating_sub(old.updated_secs) >= MAX_AGE_SECS);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn lists_real_drivers_on_this_pc() {
        assert!(!driver_files().is_empty());
    }
}
