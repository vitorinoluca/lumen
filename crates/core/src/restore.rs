//! Punto de restauración de Windows antes de cambios grandes.
//! Usa `Checkpoint-Computer` (necesita administrador y Protección del sistema activada).
//! Windows crea como máximo uno cada 24 h; si no se pudo, se informa y el cambio sigue
//! siendo reversible por el journal.
use std::{os::windows::process::CommandExt, process::Command};
use windows_sys::Win32::UI::Shell::IsUserAnAdmin;

const NO_WINDOW: u32 = 0x0800_0000;

pub fn is_admin() -> bool {
    // SAFETY: función sin argumentos ni efectos.
    unsafe { IsUserAnAdmin() != 0 }
}

/// Comando de PowerShell; las comillas simples del texto se duplican.
fn script(description: &str) -> String {
    let d = description.replace('\'', "''");
    format!("Checkpoint-Computer -Description 'lumen: {d}' -RestorePointType MODIFY_SETTINGS -WarningAction Stop -ErrorAction Stop")
}

/// Crea el punto. Err con un mensaje para mostrar al usuario si no se pudo.
pub fn create(description: &str) -> Result<(), String> {
    if !is_admin() {
        return Err("sin permisos de administrador".into());
    }
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script(description)])
        .creation_flags(NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    // Windows limita a un punto cada 24 h; es el motivo más común.
    let msg = String::from_utf8_lossy(&out.stderr);
    let first = msg.lines().find(|l| !l.trim().is_empty()).unwrap_or("error desconocido");
    Err(first.trim().chars().take(140).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_escapes_quotes() {
        let s = script("desactivar 'X'; calc");
        assert!(s.contains("lumen: desactivar ''X''; calc'"));
        assert!(s.starts_with("Checkpoint-Computer"));
    }

    #[test]
    fn admin_check_does_not_panic() {
        let _ = is_admin();
    }
}
