//! Desinstalador: lista los programas instalados, lanza su desinstalador y propone restos
//! (carpetas y claves de registro) para limpiar. Nunca borra solo: cada resto se confirma,
//! las carpetas van a la reserva y las claves se respaldan en un `.reg`.
use crate::actions::{Action, Hive, Journal};
use std::{env, io, os::windows::process::CommandExt, path::{Path, PathBuf}, process::Command};
use winreg::{enums::*, RegKey};

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

/// Nombres demasiado genéricos: coinciden con carpetas compartidas de muchas apps.
const GENERIC: &[&str] = &["microsoft", "windows", "google", "adobe", "intel", "nvidia", "amd", "apple", "java", "python", "common files"];

#[derive(Debug, Clone)]
pub struct App {
    pub key: String,
    pub hive: Hive,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub size_kb: u64,
    pub location: Option<PathBuf>,
    pub uninstall: String,
    /// `DisplayIcon` del registro (ruta, a veces con índice).
    pub icon: String,
}

impl App {
    /// De dónde sacar el ícono, en orden: el declarado y, si no, el ejecutable del desinstalador.
    pub fn icon_sources(&self) -> Vec<String> {
        let mut v = vec![];
        if !self.icon.is_empty() {
            v.push(self.icon.clone());
        }
        v.extend(crate::icons::exe_of_command(&self.uninstall));
        v
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Leftover {
    Folder(PathBuf),
    Reg(Hive, String),
}

impl Leftover {
    pub fn describe(&self) -> (&'static str, String) {
        match self {
            Leftover::Folder(p) => ("Carpeta", p.display().to_string()),
            Leftover::Reg(h, p) => ("Registro", format!("{}\\{p}", if *h == Hive::User { "HKCU" } else { "HKLM" })),
        }
    }
}

fn root(h: Hive) -> RegKey {
    RegKey::predef(if h == Hive::User { HKEY_CURRENT_USER } else { HKEY_LOCAL_MACHINE })
}

pub fn list() -> Vec<App> {
    let mut apps: Vec<App> = vec![];
    for (hive, path) in [(Hive::Machine, UNINSTALL), (Hive::Machine, UNINSTALL_32), (Hive::User, UNINSTALL)] {
        let Ok(base) = root(hive).open_subkey(path) else { continue };
        for key in base.enum_keys().flatten() {
            let Ok(k) = base.open_subkey(&key) else { continue };
            let text = |n: &str| k.get_value::<String, _>(n).unwrap_or_default().trim().to_string();
            let (name, uninstall) = (text("DisplayName"), text("UninstallString"));
            let hidden = k.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1 || !text("ParentKeyName").is_empty();
            if name.is_empty() || uninstall.is_empty() || hidden {
                continue;
            }
            let loc = text("InstallLocation").trim_matches('"').to_string();
            let app = App {
                key,
                hive,
                version: text("DisplayVersion"),
                publisher: text("Publisher"),
                size_kb: k.get_value::<u32, _>("EstimatedSize").unwrap_or(0) as u64,
                location: (!loc.is_empty()).then(|| PathBuf::from(loc)),
                icon: text("DisplayIcon"),
                name,
                uninstall,
            };
            if !apps.iter().any(|a| a.name == app.name && a.version == app.version) {
                apps.push(app);
            }
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Lanza el desinstalador propio del programa y espera a que termine.
/// El texto viene del registro de Windows, de una app que el usuario eligió desinstalar.
pub fn run_uninstall(app: &App) -> io::Result<()> {
    Command::new("cmd")
        .raw_arg(format!("/S /C \"{}\"", app.uninstall))
        .creation_flags(0x0800_0000) // sin ventana de consola; el instalador abre la suya
        .status()
        .map(|_| ())
}

fn still_installed(app: &App) -> bool {
    [UNINSTALL, UNINSTALL_32].iter().any(|p| root(app.hive).open_subkey(format!(r"{p}\{}", app.key)).is_ok())
}

/// Nombres de carpeta/clave a buscar: el nombre tal cual y sin versión ni paréntesis.
fn candidates(name: &str) -> Vec<String> {
    let full = name.trim().to_string();
    let base = full.split(" (").next().unwrap_or("").trim();
    let mut words: Vec<&str> = base.split_whitespace().collect();
    while words.len() > 1 && words.last().is_some_and(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()) || (w.starts_with('v') && w.chars().nth(1).is_some_and(|c| c.is_ascii_digit()))) {
        words.pop();
    }
    let stripped = words.join(" ");
    let mut out = vec![full, stripped];
    out.dedup();
    out.retain(|n| n.chars().count() >= 4 && !GENERIC.contains(&n.to_lowercase().as_str()));
    out
}

/// Una carpeta de instalación solo se propone si es claramente de la app: no una raíz ni algo de Windows.
fn safe_folder(p: &Path) -> bool {
    // Las rutas de Windows no distinguen mayúsculas, pero `Path::starts_with` sí: se compara en minúsculas.
    let lower = p.to_string_lossy().to_lowercase();
    let win = env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()).to_lowercase();
    p.components().count() >= 4
        && !lower.starts_with(&win)
        && !p.file_name().is_some_and(|n| GENERIC.contains(&n.to_string_lossy().to_lowercase().as_str()))
}

/// Restos que quedaron después de desinstalar. Vacío si el programa sigue instalado.
pub fn leftovers(app: &App) -> Vec<Leftover> {
    if still_installed(app) {
        return vec![];
    }
    let mut v: Vec<Leftover> = vec![];
    let mut add = |l: Leftover| {
        if !v.contains(&l) {
            v.push(l);
        }
    };
    if let Some(loc) = app.location.as_ref().filter(|l| l.is_dir() && safe_folder(l)) {
        add(Leftover::Folder(loc.clone()));
    }
    let names = candidates(&app.name);
    for var in ["APPDATA", "LOCALAPPDATA", "PROGRAMDATA"] {
        let Some(base) = env::var_os(var).map(PathBuf::from) else { continue };
        for n in &names {
            let p = base.join(n);
            if p.is_dir() {
                add(Leftover::Folder(p));
            }
        }
    }
    for (hive, base) in [(Hive::User, "Software"), (Hive::Machine, "SOFTWARE")] {
        for n in &names {
            let path = format!(r"{base}\{n}");
            if root(hive).open_subkey(&path).is_ok() {
                add(Leftover::Reg(hive, path));
            }
        }
    }
    v
}

/// Quita un resto de forma reversible. Devuelve el id del journal.
pub fn clean_leftover(j: &mut Journal, app_name: &str, l: &Leftover, stash: &Path) -> io::Result<usize> {
    let batch = stash.join(format!("restos-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis())));
    let desc = format!("Resto de {app_name}: {}", l.describe().1);
    match l {
        Leftover::Folder(p) => {
            let to = batch.join(p.file_name().unwrap_or_default());
            j.apply(&desc, Action::MoveFile { from: p.clone(), to })
        }
        Leftover::Reg(hive, path) => j.apply(&desc, Action::DeleteRegKey { hive: *hive, path: path.clone(), backup: batch.join("clave.reg") }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_strip_versions_and_skip_generic_names() {
        assert_eq!(candidates("Mozilla Firefox (x64 es-AR)"), ["Mozilla Firefox (x64 es-AR)", "Mozilla Firefox"]);
        assert_eq!(candidates("7-Zip 23.01 (x64)"), ["7-Zip 23.01 (x64)", "7-Zip"]);
        assert_eq!(candidates("Foo Bar v2.1"), ["Foo Bar v2.1", "Foo Bar"]);
        assert!(candidates("Microsoft").is_empty());
        assert!(candidates("Go").is_empty()); // demasiado corto
    }

    #[test]
    fn install_folder_safety() {
        assert!(safe_folder(Path::new(r"C:\Program Files\Foo")));
        assert!(!safe_folder(Path::new(r"C:\Program Files")));
        assert!(!safe_folder(Path::new(r"C:\Windows\System32\Foo")));
        assert!(!safe_folder(Path::new(r"C:\Program Files\Google")));
    }

    #[test]
    fn lists_installed_apps() {
        let apps = list();
        assert!(!apps.is_empty());
        assert!(apps.iter().all(|a| !a.name.is_empty() && !a.uninstall.is_empty()));
    }

    #[test]
    fn leftover_folder_is_stashed_and_restorable() {
        let base = env::temp_dir().join(format!("lumen-left-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("data").join("FooApp");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "x").unwrap();
        let mut j = Journal::open(base.join("j.json")).unwrap();
        let id = clean_leftover(&mut j, "FooApp", &Leftover::Folder(dir.clone()), &base.join("stash")).unwrap();
        assert!(!dir.exists());
        j.revert(id).unwrap();
        assert!(dir.join("a.txt").exists());
        let _ = std::fs::remove_dir_all(base);
    }
}
