//! Planes de energía de Windows, vía `powercfg`.
use std::{io, os::windows::process::CommandExt, process::Command};

pub const HIGH_PERFORMANCE: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
pub const POWER_SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";
pub const BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";

/// Nombre legible de los planes conocidos; el resto se muestra con su GUID.
pub fn name(guid: &str) -> String {
    match guid {
        HIGH_PERFORMANCE => "Alto rendimiento".into(),
        POWER_SAVER => "Ahorro de energía".into(),
        BALANCED => "Equilibrado".into(),
        other => other.into(),
    }
}

fn run(args: &[&str]) -> io::Result<String> {
    let out = Command::new("powercfg").args(args).creation_flags(0x0800_0000).output()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        Ok(text)
    } else {
        Err(io::Error::other(format!("powercfg: {}", String::from_utf8_lossy(&out.stderr).trim())))
    }
}

/// Primer GUID que aparece en el texto (sirve igual con la salida en español o inglés).
pub fn parse_guid(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_hexdigit() || c == '-'))
        .find(|t| t.len() == 36 && [8, 13, 18, 23].iter().all(|&i| t.as_bytes()[i] == b'-'))
        .map(str::to_lowercase)
}

pub fn active() -> io::Result<String> {
    parse_guid(&run(&["/getactivescheme"])?).ok_or_else(|| io::Error::other("no se pudo leer el plan de energía"))
}

pub fn set(guid: &str) -> io::Result<()> {
    run(&["/setactive", guid]).map(|_| ())
}

/// GUID del plan a usar: el base si existe en esta PC; si no, se crea una copia.
/// (Muchas notebooks modernas no traen "Alto rendimiento".)
pub fn resolve(template: &str) -> io::Result<String> {
    if run(&["/list"])?.to_lowercase().contains(template) {
        return Ok(template.into());
    }
    parse_guid(&run(&["/duplicatescheme", template])?).ok_or_else(|| io::Error::other("no se pudo crear el plan de energía"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_guid_from_english_and_spanish_output() {
        let en = "Power Scheme GUID: 381B4222-F694-41F0-9685-FF5BB260DF2E  (Balanced)";
        let es = "GUID del plan de energía: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (Alto rendimiento)";
        assert_eq!(parse_guid(en).as_deref(), Some("381b4222-f694-41f0-9685-ff5bb260df2e"));
        assert_eq!(parse_guid(es).as_deref(), Some(HIGH_PERFORMANCE));
        assert_eq!(parse_guid("sin guid acá"), None);
    }

    #[test]
    fn can_read_active_plan() {
        assert!(active().is_ok());
    }
}
