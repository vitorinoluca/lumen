//! Perfiles: modo juego (alto rendimiento y cierra una lista de procesos) y modo batería (ahorro de energía).
//! Cambiar el plan de energía se puede deshacer; cerrar procesos no.
use crate::{actions::{Action, Journal}, power, processes::Processes};
use std::{env, fs, io, path::PathBuf};

pub const GAME: &str = "Modo juego";
pub const BATTERY: &str = "Modo batería";

/// Procesos que el modo juego cierra por defecto (se puede editar el archivo).
const DEFAULT_LIST: &[&str] = &["OneDrive.exe", "Teams.exe", "ms-teams.exe", "Skype.exe", "PhoneExperienceHost.exe", "Widgets.exe"];

pub fn game_list_path() -> PathBuf {
    PathBuf::from(env::var_os("APPDATA").unwrap_or_default()).join("lumen").join("game-list.json")
}

/// Lista editable en `%APPDATA%\lumen\game-list.json`; se crea con los valores por defecto.
pub fn game_list() -> Vec<String> {
    let path = game_list_path();
    if let Some(v) = fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok()) {
        return v;
    }
    let v: Vec<String> = DEFAULT_LIST.iter().map(|s| s.to_string()).collect();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(&path, serde_json::to_vec_pretty(&v).unwrap_or_default());
    v
}

pub fn game_action() -> io::Result<Action> {
    Action::power_plan(power::HIGH_PERFORMANCE)
}

pub fn battery_action() -> io::Result<Action> {
    Action::power_plan(power::POWER_SAVER)
}

/// Cierra los procesos de la lista. Devuelve cuántos.
pub fn close_game_list(p: &mut Processes) -> usize {
    p.kill_named(&game_list())
}

/// Id de la última entrada aplicada de ese perfil cuyo plan sigue siendo el activo.
pub fn active_entry(j: &Journal, label: &str, current_plan: &str) -> Option<usize> {
    j.entries.iter().rposition(|e| {
        e.applied && e.description == label && matches!(&e.action, Action::SetPowerPlan { new, .. } if new == current_plan)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_is_on_only_while_its_plan_is_active() {
        let path = env::temp_dir().join(format!("lumen-prof-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut j = Journal::open(&path).unwrap();
        // `record` anota sin ejecutar: no toca el plan de energía real.
        j.record(GAME, Action::SetPowerPlan { new: "aaa".into(), prev: "bbb".into() }).unwrap();
        assert_eq!(active_entry(&j, GAME, "aaa"), Some(0));
        assert_eq!(active_entry(&j, GAME, "bbb"), None); // otro plan activo = perfil apagado
        assert_eq!(active_entry(&j, BATTERY, "aaa"), None);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn default_list_is_not_empty() {
        assert!(!DEFAULT_LIST.is_empty());
    }
}
