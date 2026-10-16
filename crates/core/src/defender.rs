//! Capa 1 de la revisión de seguridad: Windows Defender, que ya viene con Windows.
//! Se consulta con PowerShell (`Get-MpComputerStatus`, `Get-MpThreatDetection`, `Start-MpScan`).
//! lumen solo informa lo que Defender registra: no es un antivirus ni afirma que la PC esté limpia.
use crate::ps::{self, flag, rows, text};
use serde_json::Value;
use std::{io, os::windows::process::CommandExt, process::Command};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Status {
    pub enabled: bool,
    pub realtime: bool,
    /// "Normal", "Passive" (hay otro antivirus principal), etc.
    pub mode: String,
    pub sig_version: String,
    pub sig_updated: String,
    pub quick_scan: String,
    pub full_scan: String,
    pub tamper: bool,
    /// Otros antivirus registrados en el Centro de seguridad.
    pub other_av: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Threat {
    pub name: String,
    /// 1 bajo, 2 moderado, 4 alto, 5 grave (SeverityID de Defender); 0 = sin dato.
    pub severity: u8,
    pub resource: String,
    pub time: String,
    /// Defender la sigue considerando activa (no se pudo quitar).
    pub active: bool,
}

impl Threat {
    /// Solo la ruta del primer archivo, sin los prefijos de Defender ni la dirección de descarga.
    pub fn file(&self) -> String {
        let first = self.resource.split(';').next().unwrap_or("").trim();
        let path = first.strip_prefix("file:_").or_else(|| first.strip_prefix("webfile:_")).unwrap_or(first);
        path.split('|').next().unwrap_or("").to_string()
    }
}

pub fn parse_status(json: &str, av_json: &str) -> Option<Status> {
    let s = rows(json).into_iter().next()?;
    let other_av = rows(av_json)
        .iter()
        .map(|v| text(v, "displayName"))
        .filter(|n| !n.is_empty() && !n.to_lowercase().contains("defender"))
        .collect();
    Some(Status {
        enabled: flag(&s, "enabled"),
        realtime: flag(&s, "realtime"),
        mode: text(&s, "mode"),
        sig_version: text(&s, "sig_version"),
        sig_updated: text(&s, "sig_updated"),
        quick_scan: text(&s, "quick_scan"),
        full_scan: text(&s, "full_scan"),
        tamper: flag(&s, "tamper"),
        other_av,
    })
}

pub fn parse_threats(json: &str) -> Vec<Threat> {
    rows(json)
        .iter()
        .map(|v| Threat {
            name: text(v, "name"),
            severity: v.get("severity").and_then(Value::as_u64).unwrap_or(0) as u8,
            resource: text(v, "resource"),
            time: text(v, "time"),
            active: flag(v, "active"),
        })
        .collect()
}

const STATUS_SCRIPT: &str = "$s = Get-MpComputerStatus; \
    function D($x) { if ($x) { $x.ToString('o') } else { '' } } \
    [pscustomobject]@{ enabled=[bool]$s.AntivirusEnabled; realtime=[bool]$s.RealTimeProtectionEnabled; mode=[string]$s.AMRunningMode; \
    sig_version=[string]$s.AntivirusSignatureVersion; sig_updated=(D $s.AntivirusSignatureLastUpdated); \
    quick_scan=(D $s.QuickScanEndTime); full_scan=(D $s.FullScanEndTime); tamper=[bool]$s.IsTamperProtected } | ConvertTo-Json -Compress";

const AV_SCRIPT: &str = "Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct -ErrorAction SilentlyContinue | \
    Select-Object displayName | ConvertTo-Json -Compress";

const THREATS_SCRIPT: &str = "$t = @{}; Get-MpThreat -ErrorAction SilentlyContinue | ForEach-Object { $t[[string]$_.ThreatID] = $_ }; \
    Get-MpThreatDetection -ErrorAction SilentlyContinue | ForEach-Object { $k = [string]$_.ThreatID; \
    [pscustomobject]@{ name=[string]$t[$k].ThreatName; severity=[int]$t[$k].SeverityID; active=[bool]$t[$k].IsActive; resource=(@($_.Resources) -join '; '); \
    time=$(if ($_.InitialDetectionTime) { $_.InitialDetectionTime.ToString('o') } else { '' }) } } | ConvertTo-Json -Compress";

pub fn status() -> io::Result<Status> {
    let main = ps::run(STATUS_SCRIPT)?;
    let av = ps::run(AV_SCRIPT).unwrap_or_default();
    parse_status(&main, &av).ok_or_else(|| io::Error::other("Defender no devolvió su estado"))
}

pub fn threats() -> io::Result<Vec<Threat>> {
    Ok(parse_threats(&ps::run(THREATS_SCRIPT)?))
}

/// Abre la app Seguridad de Windows, donde se ve el historial y se resuelven las detecciones.
pub fn open_app() -> io::Result<()> {
    Command::new("cmd").args(["/C", "start", "", "windowsdefender:"]).creation_flags(0x0800_0000).spawn().map(|_| ())
}

/// Análisis rápido de Defender. Puede tardar varios minutos: llamarlo desde un hilo.
pub fn quick_scan() -> io::Result<()> {
    ps::run("Start-MpScan -ScanType QuickScan -ErrorAction Stop").map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_and_ignores_defender_in_other_av() {
        let st = r#"{"enabled":true,"realtime":true,"mode":"Normal","sig_version":"1.2.3","sig_updated":"2026-10-15T10:00:00.0000000+00:00","quick_scan":"","full_scan":"","tamper":true}"#;
        let av = r#"[{"displayName":"Windows Defender"},{"displayName":"Otro AV"}]"#;
        let s = parse_status(st, av).unwrap();
        assert!(s.enabled && s.realtime && s.tamper);
        assert_eq!(s.mode, "Normal");
        assert_eq!(s.other_av, ["Otro AV"]);
        assert!(parse_status("", "").is_none());
    }

    #[test]
    fn parses_one_or_many_threats() {
        let one = r#"{"name":"EICAR_Test_File","severity":5,"resource":"file:_C:\\x\\eicar.com","time":"2026-10-15T10:00:00Z"}"#;
        let t = parse_threats(one);
        assert_eq!((t.len(), t[0].severity, t[0].name.as_str()), (1, 5, "EICAR_Test_File"));
        assert_eq!(parse_threats(&format!("[{one},{one}]")).len(), 2);
        assert!(parse_threats("").is_empty()); // sin amenazas PowerShell no imprime nada
    }

    #[test]
    fn threat_file_drops_prefixes_and_download_url() {
        let t = Threat { resource: r"file:_C:\Users\a\x.zip; webfile:_C:\Users\a\x.zip|https://sitio/x.zip|pid:1".into(), ..Default::default() };
        assert_eq!(t.file(), r"C:\Users\a\x.zip");
        let w = Threat { resource: r"webfile:_C:\y.zip|https://sitio/y.zip".into(), ..Default::default() };
        assert_eq!(w.file(), r"C:\y.zip");
    }

    #[test]
    fn live_query_does_not_panic() {
        // En esta PC debería responder; si Defender no está, solo devuelve Err.
        let _ = status();
        let _ = threats();
    }
}
