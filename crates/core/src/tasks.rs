//! Tareas de mantenimiento de Windows: limpiar DNS, resetear la red, SFC, DISM, reparar Windows Update
//! y ver la salud del disco. Casi ninguna se puede deshacer, así que la pantalla las marca y las muestra
//! antes de ejecutar. Todas necesitan administrador salvo la lectura de salud del disco.
use crate::ps;
use serde::{Deserialize, Serialize};
use std::{io, os::windows::process::CommandExt, process::Command};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    FlushDns,
    ResetNetwork,
    Sfc,
    Dism,
    RepairUpdate,
    /// Solo lee: no cambia nada.
    DiskHealth,
}

impl Task {
    /// Qué hace, para la vista previa y el historial.
    pub fn describe(self) -> &'static str {
        match self {
            Task::FlushDns => "Vaciar la caché de DNS (ipconfig /flushdns)",
            Task::ResetNetwork => "Restablecer Winsock y TCP/IP (netsh winsock reset y netsh int ip reset). Hay que reiniciar la PC después",
            Task::Sfc => "Revisar y reparar los archivos del sistema (sfc /scannow). Tarda varios minutos",
            Task::Dism => "Reparar la imagen de Windows (DISM /Online /Cleanup-Image /RestoreHealth). Tarda varios minutos",
            Task::RepairUpdate => "Detener los servicios de Windows Update y renombrar SoftwareDistribution y catroot2 (Windows los vuelve a crear)",
            Task::DiskHealth => "Leer la salud de los discos (SMART), sin cambiar nada",
        }
    }

    pub fn read_only(self) -> bool {
        self == Task::DiskHealth
    }

    /// Ejecuta la tarea y devuelve un resumen para mostrar. Puede tardar minutos: va en un hilo.
    pub fn run(self) -> io::Result<String> {
        match self {
            Task::FlushDns => program("ipconfig", &["/flushdns"]).map(|_| "caché de DNS vaciada".into()),
            Task::ResetNetwork => {
                program("netsh", &["winsock", "reset"])?;
                program("netsh", &["int", "ip", "reset"])?;
                Ok("red restablecida: reiniciá la PC para terminar".into())
            }
            Task::Sfc => program("sfc", &["/scannow"]).map(|t| last_lines(&t, 2)),
            Task::Dism => program("dism", &["/Online", "/Cleanup-Image", "/RestoreHealth"]).map(|t| last_lines(&t, 1)),
            Task::RepairUpdate => ps::run(REPAIR_UPDATE).map(|_| "Windows Update restablecido (las carpetas viejas quedaron como .old, podés borrarlas)".into()),
            Task::DiskHealth => disk_health(),
        }
    }
}

const REPAIR_UPDATE: &str = r#"
$stamp = Get-Date -Format yyyyMMddHHmmss
try {
  Stop-Service wuauserv, bits, cryptsvc -Force -ErrorAction Stop
  foreach ($p in "$env:WINDIR\SoftwareDistribution", "$env:WINDIR\System32\catroot2") {
    if (Test-Path $p) { Rename-Item -LiteralPath $p -NewName ((Split-Path $p -Leaf) + ".old.$stamp") -ErrorAction Stop }
  }
} finally {
  Start-Service cryptsvc, bits, wuauserv -ErrorAction SilentlyContinue
}
"#;

const DISK_HEALTH: &str = r#"
Get-PhysicalDisk | ForEach-Object {
  $r = $_ | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue
  [pscustomobject]@{ name = $_.FriendlyName; health = [string]$_.HealthStatus; temp = $r.Temperature; wear = $r.Wear; hours = $r.PowerOnHours }
} | ConvertTo-Json -Compress
"#;

/// Corre un programa sin ventana. Falla si termina con error y devuelve lo que escribió.
fn program(name: &str, args: &[&str]) -> io::Result<String> {
    let out = Command::new(name).args(args).creation_flags(0x0800_0000).output()?;
    let text = decode(&out.stdout);
    if out.status.success() {
        Ok(text)
    } else {
        Err(io::Error::other(format!("{name} terminó con error: {}", last_lines(&text, 1))))
    }
}

/// SFC escribe en UTF-16 y el resto en la página de códigos de la consola: se detecta y se pasa a texto.
fn decode(bytes: &[u8]) -> String {
    if bytes.len() > 1 && bytes.iter().skip(1).step_by(2).take(20).all(|b| *b == 0) {
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Las últimas `n` líneas con texto, en una sola línea y sin pasarse de largo.
fn last_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.contains('%')).collect();
    let tail = lines[lines.len().saturating_sub(n)..].join(" ");
    tail.chars().take(180).collect()
}

fn disk_health() -> io::Result<String> {
    let json = ps::run(DISK_HEALTH)?;
    let rows = ps::rows(&json);
    if rows.is_empty() {
        return Ok("no se pudo leer la salud de los discos".into());
    }
    let parts: Vec<String> = rows
        .iter()
        .map(|d| {
            let health = match ps::text(d, "health").as_str() {
                "Healthy" => "saludable".to_string(),
                "Warning" => "con advertencias".to_string(),
                "Unhealthy" => "en mal estado".to_string(),
                other => other.to_lowercase(),
            };
            let mut s = format!("{}: {health}", ps::text(d, "name"));
            if let Some(t) = d["temp"].as_u64() {
                s += &format!(", {t} °C");
            }
            if let Some(w) = d["wear"].as_u64() {
                s += &format!(", desgaste {w} %");
            }
            if let Some(h) = d["hours"].as_u64() {
                s += &format!(", {h} horas encendido");
            }
            s
        })
        .collect();
    Ok(parts.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf16_and_keeps_the_last_meaningful_lines() {
        let utf16: Vec<u8> = "Hola\r\nFin del análisis\r\n".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let text = decode(&utf16);
        assert!(text.contains("Fin del análisis"));
        assert_eq!(last_lines("a\n\nb\nc 50%\nd\n", 2), "b d");
        assert_eq!(decode(b"texto plano"), "texto plano");
    }

    #[test]
    fn only_disk_health_is_read_only() {
        assert!(Task::DiskHealth.read_only());
        assert!(![Task::FlushDns, Task::ResetNetwork, Task::Sfc, Task::Dism, Task::RepairUpdate].iter().any(|t| t.read_only()));
    }
}
