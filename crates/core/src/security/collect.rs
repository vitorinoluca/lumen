//! Recolecta los lugares donde un programa puede quedar instalado para arrancar solo.
use super::{Finding, Fix, Severity};
use crate::{actions::Hive, icons::expand, ps, startup::Source};
use std::{env, fs, path::{Path, PathBuf}};
use winreg::{enums::*, RegKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    RunKey,
    StartupFolder,
    Task,
    Service,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::RunKey => "Inicio (registro)",
            Kind::StartupFolder => "Carpeta Inicio",
            Kind::Task => "Tarea programada",
            Kind::Service => "Servicio",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Identificador estable de la entrada.
    pub key: String,
    pub kind: Kind,
    pub name: String,
    pub location: String,
    pub command: String,
    /// Disparador (tareas) o tipo de inicio (servicios).
    pub trigger: String,
    /// Archivo que realmente se ejecuta.
    pub target: Option<PathBuf>,
    pub fix: Option<Fix>,
}

const EXTENSIONS: &[&str] = &[".exe", ".bat", ".cmd", ".com", ".dll", ".vbs", ".js", ".ps1", ".msi", ".lnk", ".scr", ".cpl"];

/// Archivo que ejecuta una línea de comando; busca en System32 y el PATH si viene sin carpeta.
pub fn target_of(command: &str) -> Option<PathBuf> {
    let mut c = expand(command.trim());
    if let Some(r) = c.strip_prefix(r"\??\") {
        c = r.to_string();
    }
    if let Some(r) = c.strip_prefix(r"\SystemRoot\") {
        c = format!(r"{}\{r}", env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()));
    }
    let exe = match c.strip_prefix('"') {
        Some(r) => r.split('"').next()?.to_string(),
        None => {
            let lower = c.to_lowercase();
            let cut = EXTENSIONS.iter().filter_map(|e| lower.find(e).map(|i| i + e.len())).min();
            match cut {
                Some(n) => c[..n].to_string(),
                None => c.split_whitespace().next()?.to_string(),
            }
        }
    };
    let path = PathBuf::from(exe.trim());
    if path.is_absolute() {
        return Some(path);
    }
    // Sin carpeta: se busca como lo haría Windows.
    let win = PathBuf::from(env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()));
    let mut dirs = vec![win.join("System32"), win.clone()];
    dirs.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    let names: Vec<PathBuf> = if path.extension().is_some() { vec![path.clone()] } else { vec![path.with_extension("exe"), path] };
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

fn root(h: Hive) -> RegKey {
    RegKey::predef(if h == Hive::User { HKEY_CURRENT_USER } else { HKEY_LOCAL_MACHINE })
}

fn entry(kind: Kind, key: String, name: String, location: String, command: String, trigger: String, fix: Option<Fix>) -> Entry {
    Entry { target: target_of(&command), key, kind, name, location, command, trigger, fix }
}

/// Claves Run y RunOnce de usuario y equipo (incluida la vista de 32 bits).
pub fn run_keys() -> Vec<Entry> {
    let run = r"Software\Microsoft\Windows\CurrentVersion";
    let wow = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion";
    let places = [
        (Hive::User, "HKCU", format!(r"{run}\Run"), Some(Source::RunUser)),
        (Hive::User, "HKCU", format!(r"{run}\RunOnce"), None),
        (Hive::Machine, "HKLM", format!(r"{run}\Run"), Some(Source::RunMachine)),
        (Hive::Machine, "HKLM", format!(r"{run}\RunOnce"), None),
        (Hive::Machine, "HKLM", format!(r"{wow}\Run"), None),
        (Hive::Machine, "HKLM", format!(r"{wow}\RunOnce"), None),
    ];
    let mut out = vec![];
    for (hive, label, path, source) in places {
        let Ok(k) = root(hive).open_subkey(&path) else { continue };
        for (name, _) in k.enum_values().flatten() {
            let command: String = k.get_value(&name).unwrap_or_default();
            let location = format!(r"{label}\{path}");
            let key = format!("run:{}:{}", location.to_lowercase(), name.to_lowercase());
            let fix = source.map(|s| Fix::Startup { source: s, name: name.clone() });
            out.push(entry(Kind::RunKey, key, name, location, command, String::new(), fix));
        }
    }
    out
}

/// Carpetas Inicio del usuario y global. Los accesos directos (.lnk) se resuelven a su destino.
pub fn startup_folders() -> Vec<Entry> {
    let sub = r"Microsoft\Windows\Start Menu\Programs\Startup";
    let dirs = [("usuario", env::var_os("APPDATA"), true), ("global", env::var_os("ProgramData"), false)];
    let mut files = vec![];
    for (scope, base, user) in dirs {
        let Some(base) = base else { continue };
        for e in fs::read_dir(PathBuf::from(base).join(sub)).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.eq_ignore_ascii_case("desktop.ini") {
                files.push((scope, user, name, e.path()));
            }
        }
    }
    let lnks: Vec<String> = files.iter().filter(|f| f.3.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk"))).map(|f| f.3.display().to_string()).collect();
    let resolved = resolve_shortcuts(&lnks);
    files
        .into_iter()
        .map(|(scope, user, name, path)| {
            let command = resolved.iter().find(|(p, _)| *p == path.display().to_string()).map(|(_, c)| c.clone()).unwrap_or_else(|| path.display().to_string());
            let key = format!("startup:{scope}:{}", name.to_lowercase());
            let fix = user.then(|| Fix::Startup { source: Source::FolderUser, name: name.clone() });
            entry(Kind::StartupFolder, key, name, format!("Carpeta Inicio ({scope})"), command, String::new(), fix)
        })
        .collect()
}

/// (acceso directo, "destino argumentos"). Una sola llamada a PowerShell para todos.
fn resolve_shortcuts(paths: &[String]) -> Vec<(String, String)> {
    if paths.is_empty() {
        return vec![];
    }
    let list = paths.iter().map(|p| ps::quote(p)).collect::<Vec<_>>().join(",");
    let script = format!(
        "$sh = New-Object -ComObject WScript.Shell; @({list}) | ForEach-Object {{ $l = $sh.CreateShortcut($_); \
         [pscustomobject]@{{ path=[string]$_; target=[string]$l.TargetPath; args=[string]$l.Arguments }} }} | ConvertTo-Json -Compress"
    );
    ps::rows(&ps::run(&script).unwrap_or_default())
        .iter()
        .map(|r| (ps::text(r, "path"), format!("{} {}", ps::text(r, "target"), ps::text(r, "args")).trim().to_string()))
        .collect()
}

/// Tareas programadas activas con su acción, argumentos y disparador.
pub fn tasks() -> Vec<Entry> {
    let script = "Get-ScheduledTask | Where-Object { $_.State -ne 'Disabled' } | ForEach-Object { $t = $_; $i = 0; \
        foreach ($a in $t.Actions) { $i++; if ($a.Execute) { [pscustomobject]@{ path=[string]$t.TaskPath; name=[string]$t.TaskName; idx=$i; \
        exe=[string]$a.Execute; args=[string]$a.Arguments; trig=(@($t.Triggers | ForEach-Object { $_.CimClass.CimClassName -replace 'MSFT_Task','' -replace 'Trigger','' }) -join ', ') } } } } \
        | ConvertTo-Json -Compress";
    ps::rows(&ps::run(script).unwrap_or_default())
        .iter()
        .map(|r| {
            let (path, name, exe) = (ps::text(r, "path"), ps::text(r, "name"), ps::text(r, "exe"));
            let command = format!("{exe} {}", ps::text(r, "args")).trim().to_string();
            let idx = r.get("idx").and_then(|v| v.as_u64()).unwrap_or(1);
            let key = format!("task:{}{}|{idx}", path.to_lowercase(), name.to_lowercase());
            entry(Kind::Task, key, format!("{path}{name}"), "Programador de tareas".into(), command, ps::text(r, "trig"), Some(Fix::Task { path, name }))
        })
        .collect()
}

/// Servicios de Windows (no controladores) con inicio automático o manual.
/// Los servicios que cuelgan de svchost se miran por su `ServiceDll`, que es lo que de verdad se carga.
pub fn services() -> Vec<Entry> {
    let Ok(base) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SYSTEM\CurrentControlSet\Services") else { return vec![] };
    let mut out = vec![];
    for name in base.enum_keys().flatten() {
        let Ok(k) = base.open_subkey(&name) else { continue };
        let (ty, start): (u32, u32) = (k.get_value("Type").unwrap_or(0), k.get_value("Start").unwrap_or(4));
        let image: String = k.get_value("ImagePath").unwrap_or_default();
        if ty & 0x30 == 0 || !(2..=3).contains(&start) || image.is_empty() {
            continue;
        }
        let mut command = image.clone();
        if image.to_lowercase().contains("svchost") {
            if let Ok(dll) = k.open_subkey("Parameters").and_then(|p| p.get_value::<String, _>("ServiceDll")) {
                command = format!("{image} [{dll}]");
            }
        }
        let trigger = if start == 2 { "Automático" } else { "Manual" }.to_string();
        let mut e = entry(Kind::Service, format!("service:{}", name.to_lowercase()), name.clone(), "Servicio".into(), command.clone(), trigger, Some(Fix::Service { name }));
        if let Some(dll) = command.split('[').nth(1).map(|d| d.trim_end_matches(']')) {
            e.target = target_of(dll); // lo que realmente se carga dentro de svchost
        }
        out.push(e);
    }
    out
}

/// Si dos entradas comparten identificador, la segunda lleva un sufijo: cada una se compara por separado.
pub fn dedupe_keys(v: &mut [Entry]) {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for e in v {
        let n = seen.entry(e.key.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            e.key = format!("{}#{n}", e.key);
        }
    }
}

pub fn all() -> Vec<Entry> {
    let mut v = run_keys();
    v.extend(startup_folders());
    v.extend(services());
    v.extend(tasks());
    dedupe_keys(&mut v);
    v
}

// ---------- puntos que son sospechosos por sí mismos ----------

const WINLOGON: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon";
const IFEO: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options";

/// Winlogon: `Shell` debe ser explorer.exe y `Userinit` el userinit.exe de System32.
pub fn winlogon_findings(shell: &str, userinit: &str, windir: &str) -> Vec<Finding> {
    let mut out = vec![];
    let expected = format!(r"{windir}\system32\userinit.exe").to_lowercase();
    let mut check = |name: &str, value: &str, ok: bool, restore: String| {
        if !ok {
            out.push(Finding {
                key: format!("winlogon:{}", name.to_lowercase()),
                severity: Severity::High,
                title: format!("Winlogon: «{name}» fue modificado"),
                detail: format!("Valor actual: {value}\nValor esperado: {restore}"),
                decoded: None,
                fix: Some(Fix::RegText { hive: Hive::Machine, path: WINLOGON.into(), name: name.into(), restore: Some(restore) }),
            });
        }
    };
    check("Shell", shell, shell.trim().eq_ignore_ascii_case("explorer.exe"), "explorer.exe".into());
    let u = userinit.trim().to_lowercase();
    check("Userinit", userinit, u == expected || u == format!("{expected},"), format!(r"{windir}\system32\userinit.exe,"));
    out
}

/// `Image File Execution Options`: un `Debugger` hace que Windows lance otro programa en lugar del original.
fn ifeo_findings() -> Vec<Finding> {
    let mut out = vec![];
    for (view, path) in [("64", IFEO.to_string()), ("32", IFEO.replace(r"SOFTWARE\Microsoft", r"SOFTWARE\WOW6432Node\Microsoft"))] {
        let Ok(base) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(&path) else { continue };
        for exe in base.enum_keys().flatten() {
            let Ok(dbg) = base.open_subkey(&exe).and_then(|k| k.get_value::<String, _>("Debugger")) else { continue };
            out.push(Finding {
                key: format!("ifeo:{view}:{}", exe.to_lowercase()),
                severity: Severity::High,
                title: format!("{exe} se reemplaza por otro programa (IFEO)"),
                detail: format!("Cada vez que se abre {exe}, Windows ejecuta: {dbg}"),
                decoded: None,
                fix: Some(Fix::RegText { hive: Hive::Machine, path: format!(r"{path}\{exe}"), name: "Debugger".into(), restore: None }),
            });
        }
    }
    out
}

/// Suscripciones WMI: se usan para ejecutar código sin dejar nada en Inicio. Se ignoran las que trae Windows.
pub fn wmi_findings(json: &str) -> Vec<Finding> {
    ps::rows(json)
        .iter()
        .filter_map(|r| {
            let (kind, name, detail) = (ps::text(r, "kind"), ps::text(r, "name"), ps::text(r, "detail"));
            if name.contains("SCM Event Log") || detail.contains("SCM Event Log") || name.is_empty() && detail.is_empty() {
                return None;
            }
            let what = match kind.as_str() {
                "filter" => "filtro de eventos",
                "consumer" => "acción",
                _ => "vínculo",
            };
            Some(Finding {
                key: format!("wmi:{kind}:{}", name.to_lowercase()),
                severity: Severity::High,
                title: format!("WMI: {what} «{name}»"),
                detail: if detail.is_empty() { "Sin detalle".into() } else { detail },
                decoded: None,
                fix: None,
            })
        })
        .collect()
}

fn wmi_json() -> String {
    let ns = r"root\subscription";
    let script = format!(
        "$f = Get-CimInstance -Namespace {ns} -ClassName __EventFilter -ErrorAction SilentlyContinue | ForEach-Object {{ [pscustomobject]@{{ kind='filter'; name=[string]$_.Name; detail=[string]$_.Query }} }}; \
         $c = Get-CimInstance -Namespace {ns} -ClassName __EventConsumer -ErrorAction SilentlyContinue | ForEach-Object {{ [pscustomobject]@{{ kind='consumer'; name=[string]$_.Name; \
         detail=((@($_.CommandLineTemplate, $_.ExecutablePath, $_.ScriptText, $_.ScriptFileName) | Where-Object {{ $_ }}) -join ' ') }} }}; \
         $b = Get-CimInstance -Namespace {ns} -ClassName __FilterToConsumerBinding -ErrorAction SilentlyContinue | ForEach-Object {{ [pscustomobject]@{{ kind='binding'; name=[string]$_.Filter.Name; detail=[string]$_.Consumer.Name }} }}; \
         @($f) + @($c) + @($b) | Where-Object {{ $_ }} | ConvertTo-Json -Compress"
    );
    ps::run(&script).unwrap_or_default()
}

pub fn special_findings() -> Vec<Finding> {
    let w = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(WINLOGON);
    let get = |n: &str| w.as_ref().ok().and_then(|k| k.get_value::<String, _>(n).ok()).unwrap_or_default();
    let windir = env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let mut v = winlogon_findings(&get("Shell"), &get("Userinit"), &windir);
    v.extend(ifeo_findings());
    v.extend(wmi_findings(&wmi_json()));
    v
}

pub fn is_under(path: &Path, dir_env: &str) -> bool {
    env::var(dir_env).is_ok_and(|d| path.to_string_lossy().to_lowercase().starts_with(&d.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_executable_in_a_command_line() {
        assert_eq!(target_of(r#""C:\Program Files\X\a.exe" /s"#), Some(PathBuf::from(r"C:\Program Files\X\a.exe")));
        assert_eq!(target_of(r"C:\Program Files\X\a.exe -k net"), Some(PathBuf::from(r"C:\Program Files\X\a.exe")));
        assert_eq!(target_of(r"C:\a\b.dll,Func arg"), Some(PathBuf::from(r"C:\a\b.dll")));
        assert!(target_of("cmd.exe /c algo").is_some_and(|p| p.ends_with("cmd.exe")));
        assert!(target_of("programa-que-no-existe.exe").is_none());
        assert!(target_of("").is_none());
    }

    #[test]
    fn duplicate_keys_get_a_suffix() {
        let mk = |k: &str| entry(Kind::RunKey, k.into(), "n".into(), "l".into(), "c.exe".into(), String::new(), None);
        let mut v = vec![mk("a"), mk("a"), mk("b"), mk("a")];
        dedupe_keys(&mut v);
        assert_eq!(v.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["a", "a#2", "b", "a#3"]);
    }

    #[test]
    fn winlogon_accepts_the_defaults_and_flags_changes() {
        let w = r"C:\WINDOWS";
        assert!(winlogon_findings("explorer.exe", r"C:\WINDOWS\system32\userinit.exe,", w).is_empty());
        assert!(winlogon_findings("Explorer.exe", r"c:\windows\system32\userinit.exe", w).is_empty());
        let bad = winlogon_findings(r"explorer.exe, malo.exe", r"C:\WINDOWS\system32\userinit.exe,C:\x\evil.exe", w);
        assert_eq!(bad.len(), 2);
        assert!(bad.iter().all(|f| f.severity == Severity::High && f.fix.is_some()));
    }

    #[test]
    fn wmi_ignores_windows_defaults() {
        let json = r#"[{"kind":"filter","name":"SCM Event Log Filter","detail":"x"},{"kind":"consumer","name":"Evil","detail":"powershell -enc AAA"},{"kind":"binding","name":"SCM Event Log Filter","detail":"SCM Event Log Consumer"}]"#;
        let f = wmi_findings(json);
        assert_eq!(f.len(), 1);
        assert!(f[0].title.contains("Evil") && f[0].severity == Severity::High);
        assert!(wmi_findings("").is_empty());
    }

    #[test]
    fn collects_real_entries_from_this_pc() {
        let all = all();
        assert!(all.iter().any(|e| e.kind == Kind::Service), "debe haber servicios");
        assert!(all.iter().all(|e| !e.key.is_empty() && !e.command.is_empty()));
    }
}
