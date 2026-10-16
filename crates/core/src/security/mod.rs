//! Revisión de seguridad local: detecta comportamiento sospechoso que Windows Defender pasa por alto.
//! Solo lectura: no cambia nada sin que el usuario lo confirme. Sin IA ni servicios externos.
//!
//! Funciona con una línea base: el primer escaneo guarda lo que hay y los siguientes avisan
//! solo de lo nuevo o modificado. Lo que es sospechoso por sí mismo (reverse shells, procesos
//! falsos, Defender apagado) se informa siempre, hasta que el usuario lo ignore.
use crate::{actions::Hive, startup::Source};
use baseline::{Baseline, Record};
use collect::Entry;
use sha2::{Digest, Sha256};
use signature::Sig;
use std::{collections::{BTreeMap, HashMap}, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

pub mod analyze;
pub mod baseline;
pub mod collect;
pub mod signature;

/// Orden de aparición: alta primero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    High,
    Medium,
    Low,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::High => "alta",
            Severity::Medium => "media",
            Severity::Low => "baja",
        }
    }
}

/// Cómo desactivar el origen de un hallazgo. Siempre con confirmación y reversible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// Entrada de inicio (clave Run del usuario o del equipo, o carpeta Inicio del usuario).
    Startup { source: Source, name: String },
    /// Tarea programada.
    Task { path: String, name: String },
    /// Servicio: se pone en "deshabilitado".
    Service { name: String },
    /// Valor de registro de texto: se borra (`restore: None`) o se vuelve al valor esperado.
    RegText { hive: Hive, path: String, name: String, restore: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Identificador estable: sirve para ignorar el hallazgo y reconocerlo en el próximo escaneo.
    pub key: String,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    /// Comando real, si estaba codificado en base64.
    pub decoded: Option<String>,
    pub fix: Option<Fix>,
}

/// Una entrada recolectada junto con lo que se guarda de ella.
pub struct Scanned {
    pub entry: Entry,
    pub record: Record,
}

#[derive(Debug, Default)]
pub struct ScanResult {
    pub findings: Vec<Finding>,
    /// Era la primera vez: solo se guardó la línea base.
    pub first_run: bool,
    pub entries: usize,
    /// Avisos que no son hallazgos (ej. "las exclusiones de Defender necesitan administrador").
    pub notes: Vec<String>,
    /// Estado actual de cada entrada, para poder aceptarla al ignorar un hallazgo.
    pub records: BTreeMap<String, Record>,
}

fn is_user_path(p: &str) -> bool {
    let l = p.to_lowercase();
    ["\\users\\", "\\appdata\\", "\\temp\\", "\\programdata\\"].iter().any(|d| l.contains(d))
}

fn short(sha: &str) -> String {
    sha.chars().take(16).collect()
}

/// Lo nuevo o modificado respecto de la línea base. Alta no corresponde acá: una entrada común
/// es media si no está firmada o vive en una carpeta de usuario, y baja si la firma un editor válido.
pub fn persistence_findings(scanned: &[Scanned], base: &Baseline) -> Vec<Finding> {
    scanned
        .iter()
        .filter_map(|s| {
            let (e, r) = (&s.entry, &s.record);
            let (prefix, what) = match base.entries.get(&e.key) {
                None => ("new", "Nuevo"),
                Some(old) if old.fingerprint() != r.fingerprint() => ("mod", "Modificado"),
                Some(_) => return None,
            };
            let severity = if r.valid && !is_user_path(&r.path) { Severity::Low } else { Severity::Medium };
            let mut detail = format!("{}\n{}\nFirma: {}", e.location, e.command, r.sig);
            if !r.sha256.is_empty() {
                detail.push_str(&format!("\nSHA-256: {}…", short(&r.sha256)));
            }
            if !e.trigger.is_empty() {
                detail.push_str(&format!("\nDisparador: {}", e.trigger));
            }
            Some(Finding { key: format!("{prefix}:{}", e.key), severity, title: format!("{what} · {}: {}", e.kind.label(), e.name), detail, decoded: None, fix: e.fix.clone() })
        })
        .collect()
}

fn cmd_id(cmd: &str) -> String {
    Sha256::digest(cmd.as_bytes()).iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// Escanea y compara con la línea base. Tarda unos segundos (consulta a Windows varias veces):
/// llamarlo desde un hilo. En el primer escaneo guarda la línea base y solo informa lo que es
/// sospechoso por sí mismo.
pub fn scan() -> ScanResult {
    let own = std::process::id();
    let mut result = ScanResult::default();
    let loaded = Baseline::load();
    result.first_run = loaded.is_none();
    let mut base = loaded.unwrap_or_default();

    // Cada consulta a PowerShell tarda ~1 s: se lanzan todas a la vez.
    let (entries, special, procs, conns, status, exclusions, updates) = std::thread::scope(|s| {
        let (a, b, c, d) = (s.spawn(collect::all), s.spawn(collect::special_findings), s.spawn(analyze::processes), s.spawn(analyze::connections));
        let (e, f, g) = (s.spawn(crate::defender::status), s.spawn(analyze::defender_exclusions), s.spawn(analyze::update_findings));
        (a.join().unwrap_or_default(), b.join().unwrap_or_default(), c.join().unwrap_or_default(), d.join().unwrap_or_default(), e.join().ok().and_then(Result::ok), f.join().ok().flatten(), g.join().unwrap_or_default())
    });

    // 1. Persistencia, con hash y firma. La firma solo se pide de lo que no está ya en la línea base.
    result.entries = entries.len();
    // Muchos servicios comparten el mismo ejecutable (svchost.exe): se calcula una vez por archivo.
    let mut hashes: HashMap<PathBuf, String> = HashMap::new();
    let hashed: Vec<(Entry, String)> = entries
        .into_iter()
        .map(|e| {
            let sha = e.target.as_ref().map(|t| hashes.entry(t.clone()).or_insert_with(|| signature::sha256(t).unwrap_or_default()).clone()).unwrap_or_default();
            (e, sha)
        })
        .collect();
    let needs_sig = |e: &Entry, sha: &str| base.entries.get(&e.key).is_none_or(|old| old.sha256 != sha || old.command != e.command);
    let mut to_verify: Vec<PathBuf> = hashed.iter().filter(|(e, sha)| needs_sig(e, sha)).filter_map(|(e, _)| e.target.clone()).collect();

    // 2. Procesos y conexiones.
    to_verify.extend(analyze::paths_to_verify(&procs, &conns));
    to_verify.extend(analyze::lookalike_paths(&procs));
    to_verify.sort();
    to_verify.dedup();
    let sigs: HashMap<PathBuf, Sig> = signature::signatures(&to_verify);

    let scanned: Vec<Scanned> = hashed
        .into_iter()
        .map(|(entry, sha)| {
            let old = base.entries.get(&entry.key).filter(|o| o.sha256 == sha && o.command == entry.command);
            let (sig, valid) = match (old, entry.target.as_ref().and_then(|t| sigs.get(t))) {
                (Some(o), _) => (o.sig.clone(), o.valid),
                (None, Some(s)) => (s.label(), s.is_valid()),
                (None, None) => (Sig::Unknown.label(), false),
            };
            let record = Record { command: entry.command.clone(), path: entry.target.as_ref().map(|t| t.display().to_string()).unwrap_or_default(), sha256: sha, sig, valid };
            Scanned { entry, record }
        })
        .collect();
    result.records = scanned.iter().map(|s| (s.entry.key.clone(), s.record.clone())).collect();

    // 3. Lo nuevo o modificado (no en el primer escaneo).
    if !result.first_run {
        result.findings.extend(persistence_findings(&scanned, &base));
    }
    // 4. Lo que es sospechoso por sí mismo.
    result.findings.extend(special);
    for s in &scanned {
        if let Some(o) = analyze::obfuscation(&s.entry.command) {
            result.findings.push(analyze::obfuscation_finding(format!("obf:{}", s.entry.key), &s.entry.kind.label().to_lowercase(), &s.entry.command, &o));
        }
    }
    for p in procs.iter().filter(|p| p.pid != own && p.ppid != own) {
        if let Some(o) = analyze::obfuscation(&p.cmd) {
            let key = format!("obf:proc:{}:{}", p.name.to_lowercase(), cmd_id(&p.cmd));
            result.findings.push(analyze::obfuscation_finding(key, &format!("el proceso {}", p.name), &p.cmd, &o));
        }
    }
    result.findings.extend(analyze::reverse_shells(&procs, &conns, &sigs, own));
    result.findings.extend(analyze::fake_system_processes(&procs, &sigs, &analyze::windir()));

    // 5. Defender y Windows Update.
    if let Some(st) = status {
        result.findings.extend(analyze::defender_state_findings(st.realtime, st.enabled, st.mode == "Passive"));
    }
    result.findings.extend(updates);
    let mut changed = false;
    match exclusions {
        Some(list) => {
            if !base.exclusions_known {
                // Primera vez que se pueden leer: lo que ya había queda como línea base.
                if !result.first_run && !list.is_empty() {
                    result.notes.push(format!("Se guardaron {} exclusiones de Defender que ya existían como línea base.", list.len()));
                }
                base.exclusions = list;
                base.exclusions_known = true;
                changed = true;
            } else {
                result.findings.extend(list.iter().filter(|x| !base.exclusions.contains(x)).map(|x| analyze::exclusion_finding(x)));
            }
        }
        None => result.notes.push("Para revisar las exclusiones de Defender, lumen tiene que abrirse como administrador.".into()),
    }

    // 6. Línea base del primer escaneo.
    if result.first_run {
        base.created_secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        base.entries = result.records.clone();
        result.notes.push(format!("Primera revisión: se guardó la línea base con {} entradas. Desde ahora lumen avisa solo de lo nuevo o modificado.", result.entries));
        changed = true;
    }
    if changed {
        let _ = base.save();
    }

    // 7. Lo que el usuario ya ignoró no se vuelve a mostrar.
    result.findings.retain(|f| !base.ignored.contains(&f.key));
    result.findings.sort_by(|a, b| a.severity.cmp(&b.severity).then_with(|| a.title.cmp(&b.title)));
    result
}

/// Ignorar un hallazgo: no vuelve a aparecer y, si era una entrada nueva o modificada, pasa a la línea base.
pub fn ignore(result: &ScanResult, key: &str) -> std::io::Result<()> {
    let mut base = Baseline::load().unwrap_or_default();
    base.ignored.insert(key.to_string());
    if let Some(entry_key) = key.strip_prefix("new:").or_else(|| key.strip_prefix("mod:")) {
        if let Some(r) = result.records.get(entry_key) {
            base.entries.insert(entry_key.to_string(), r.clone());
        }
    }
    base.save()
}

#[cfg(test)]
mod tests {
    use super::*;
    use collect::Kind;

    fn scanned(key: &str, command: &str, sha: &str, path: &str, valid: bool) -> Scanned {
        Scanned {
            entry: Entry { key: key.into(), kind: Kind::RunKey, name: key.into(), location: "HKCU\\Run".into(), command: command.into(), trigger: String::new(), target: None, fix: None },
            record: Record { command: command.into(), path: path.into(), sha256: sha.into(), sig: if valid { "firmado por X".into() } else { "sin firma".into() }, valid },
        }
    }

    #[test]
    fn diff_reports_only_new_or_modified_entries() {
        let a = scanned("a", "a.exe", "11", r"C:\Program Files\A\a.exe", true);
        let mut base = Baseline::default();
        base.entries.insert("a".into(), a.record.clone());
        // igual -> nada
        assert!(persistence_findings(&[scanned("a", "a.exe", "11", r"C:\Program Files\A\a.exe", true)], &base).is_empty());
        // nuevo, firmado y en Program Files -> baja
        let f = persistence_findings(&[scanned("b", "b.exe", "22", r"C:\Program Files\B\b.exe", true)], &base);
        assert_eq!((f.len(), f[0].severity, f[0].key.as_str()), (1, Severity::Low, "new:b"));
        // nuevo, sin firma -> media; firmado pero en AppData -> media
        let f = persistence_findings(&[scanned("c", "c.exe", "33", r"C:\Program Files\C\c.exe", false), scanned("d", "d.exe", "44", r"C:\Users\x\AppData\Local\d.exe", true)], &base);
        assert!(f.iter().all(|x| x.severity == Severity::Medium) && f.len() == 2);
        // el archivo cambió de contenido -> modificado
        let f = persistence_findings(&[scanned("a", "a.exe", "99", r"C:\Program Files\A\a.exe", true)], &base);
        assert_eq!((f.len(), f[0].key.as_str()), (1, "mod:a"));
        assert!(f[0].title.starts_with("Modificado"));
    }

    #[test]
    fn user_paths_are_detected() {
        assert!(is_user_path(r"C:\Users\a\Downloads\x.exe"));
        assert!(is_user_path(r"C:\ProgramData\x\y.exe"));
        assert!(!is_user_path(r"C:\Program Files\X\x.exe"));
        assert!(!is_user_path(r"C:\Windows\System32\svchost.exe"));
    }
}
