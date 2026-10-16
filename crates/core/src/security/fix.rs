//! Acciones sobre un hallazgo: deshabilitar (con respaldo) y restaurar. Todo pasa por el journal,
//! así que lo que deshabilita lumen siempre se puede volver a habilitar.
use super::{Finding, Fix};
use crate::actions::{Action, Hive, Journal};
use std::{env, io, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

fn description(key: &str) -> String {
    format!("Seguridad: {key}")
}

fn backup_path(name: &str) -> PathBuf {
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    PathBuf::from(env::var_os("APPDATA").unwrap_or_default()).join("lumen").join("backups").join(format!("tarea-{safe}-{ms}.xml"))
}

/// La acción que desactiva el origen del hallazgo. Lee el estado actual para poder revertirlo.
pub fn action_for(fix: &Fix) -> Action {
    match fix {
        Fix::Startup { source, name } => Action::SetStartup { source: *source, name: name.clone(), enable: false },
        Fix::Service { name } => Action::set_dword(Hive::Machine, &format!(r"SYSTEM\CurrentControlSet\Services\{name}"), "Start", 4),
        Fix::Task { path, name } => Action::SetTaskEnabled { path: path.clone(), name: name.clone(), enable: false, backup: backup_path(name) },
        Fix::RegText { hive, path, name, restore } => Action::set_text(*hive, path, name, restore.clone()),
    }
}

/// Deshabilita el origen del hallazgo. Devuelve el id del journal.
pub fn disable(j: &mut Journal, f: &Finding) -> io::Result<usize> {
    let fix = f.fix.as_ref().ok_or_else(|| io::Error::other("este hallazgo no se puede deshabilitar desde lumen"))?;
    j.apply(&description(&f.key), action_for(fix))
}

/// Id del journal si lumen deshabilitó este hallazgo y sigue deshabilitado.
pub fn disabled_id(j: &Journal, key: &str) -> Option<usize> {
    let d = description(key);
    j.entries.iter().rposition(|e| e.applied && e.description == d)
}

/// Deshace lo que deshabilitó lumen.
pub fn restore(j: &mut Journal, key: &str) -> io::Result<()> {
    let id = disabled_id(j, key).ok_or_else(|| io::Error::other("lumen no deshabilitó esto, no hay nada que restaurar"))?;
    j.revert(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{security::Severity, startup::Source};
    use winreg::{enums::*, RegKey};

    fn finding(key: &str, fix: Fix) -> Finding {
        Finding { key: key.into(), severity: Severity::High, title: "t".into(), detail: String::new(), decoded: None, fix: Some(fix) }
    }

    #[test]
    fn maps_each_fix_to_a_reversible_action() {
        assert!(matches!(action_for(&Fix::Startup { source: Source::RunUser, name: "x".into() }), Action::SetStartup { enable: false, .. }));
        match action_for(&Fix::Service { name: "Foo".into() }) {
            Action::SetDword { hive, path, new, .. } => assert!(hive == Hive::Machine && path.ends_with(r"Services\Foo") && new == 4),
            other => panic!("{other:?}"),
        }
        match action_for(&Fix::Task { path: r"\Dir\".into(), name: "T".into(), }) {
            Action::SetTaskEnabled { enable, backup, .. } => assert!(!enable && backup.to_string_lossy().contains("backups")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn disable_and_restore_a_registry_value_through_the_journal() {
        let path = format!(r"Software\lumen-test-fix-{}", std::process::id());
        let root = RegKey::predef(HKEY_CURRENT_USER);
        root.create_subkey(&path).unwrap().0.set_value("Debugger", &"malo.exe").unwrap();
        let tmp = std::env::temp_dir().join(format!("lumen-fix-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let mut j = Journal::open(&tmp).unwrap();
        let f = finding("ifeo:test", Fix::RegText { hive: Hive::User, path: path.clone(), name: "Debugger".into(), restore: None });

        assert!(disabled_id(&j, &f.key).is_none());
        disable(&mut j, &f).unwrap();
        assert!(root.open_subkey(&path).unwrap().get_value::<String, _>("Debugger").is_err()); // se quitó
        assert!(disabled_id(&j, &f.key).is_some());

        restore(&mut j, &f.key).unwrap();
        let back: String = root.open_subkey(&path).unwrap().get_value("Debugger").unwrap();
        assert_eq!(back, "malo.exe"); // vuelve exactamente a como estaba
        assert!(restore(&mut j, &f.key).is_err()); // ya está restaurado

        let _ = root.delete_subkey_all(&path);
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn findings_without_a_fix_cannot_be_disabled() {
        let tmp = std::env::temp_dir().join(format!("lumen-fix2-{}.json", std::process::id()));
        let mut j = Journal::open(&tmp).unwrap();
        let f = Finding { key: "k".into(), severity: Severity::High, title: "t".into(), detail: String::new(), decoded: None, fix: None };
        assert!(disable(&mut j, &f).is_err());
        let _ = std::fs::remove_file(tmp);
    }
}
