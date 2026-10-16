//! Opciones de la sección Actualizaciones: cómo se actualiza Windows. Se usan las directivas de Windows Update
//! (registro del equipo), así que se aplican y se deshacen con valores guardados, igual que el resto.
use crate::{
    actions::{Action, Hive},
    plan::{Item, Risk, Step},
};

const WU: &str = r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate";
const AU: &str = r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU";

fn dword(path: &str, name: &str, v: u32) -> Step {
    Step { action: Action::set_dword(Hive::Machine, path, name, v) }
}

/// Modo actual según las directivas: 0 = por defecto, 1 = solo seguridad, 2 = pausadas.
fn current() -> usize {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    let read = |path: &str, name: &str| RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(path).and_then(|k| k.get_value::<u32, _>(name)).unwrap_or(0);
    if read(AU, "NoAutoUpdate") == 1 {
        2
    } else if read(WU, "DeferFeatureUpdates") == 1 {
        1
    } else {
        0
    }
}

pub fn items() -> Vec<Item> {
    let modes = [
        ("upd:default", "Por defecto", "Windows decide cuándo y qué instalar. Recibís parches y novedades.", Risk::Safe, vec![dword(WU, "DeferFeatureUpdates", 0), dword(AU, "NoAutoUpdate", 0)]),
        (
            "upd:security",
            "Solo seguridad",
            "Los parches de seguridad llegan igual; las versiones nuevas de Windows se posponen un año.",
            Risk::Review,
            vec![dword(WU, "DeferFeatureUpdates", 1), dword(WU, "DeferFeatureUpdatesPeriodInDays", 365), dword(AU, "NoAutoUpdate", 0)],
        ),
        (
            "upd:paused",
            "Pausadas",
            "Windows no busca ni instala nada solo. Te quedás sin parches de seguridad hasta volver a otro modo.",
            Risk::Review,
            vec![dword(WU, "DeferFeatureUpdates", 0), dword(AU, "NoAutoUpdate", 1)],
        ),
    ];
    let now = current();
    modes
        .into_iter()
        .enumerate()
        .map(|(i, (id, title, desc, risk, steps))| {
            let mut it = Item::new(id, title, desc, risk, steps);
            it.group = 1;
            it.done = i == now;
            it
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_one_mode_is_current_and_they_form_a_single_choice() {
        let items = items();
        assert_eq!(items.len(), 3);
        assert_eq!(items.iter().filter(|i| i.done).count(), 1);
        assert!(items.iter().all(|i| i.group == 1 && !i.default_checked()));
    }
}
