//! Presets de la pantalla Inicio: qué opciones se marcan en cada nivel.
//! Mínimo = solo privacidad segura. Recomendado = todo lo verde. Avanzado = lo recomendado más lo amarillo que se puede deshacer.
//! Nunca entran las opciones que se eligen una sola (energía, DNS, actualizaciones), las apps de inicio ni el modo juego:
//! dependen de cómo usás la PC. Tampoco lo amarillo que no se puede deshacer (quitar apps, resetear la red).
use crate::plan::{Item, Risk};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    Minimal,
    Recommended,
    Advanced,
}

impl Preset {
    pub fn from_index(i: i32) -> Self {
        match i {
            0 => Preset::Minimal,
            2 => Preset::Advanced,
            _ => Preset::Recommended,
        }
    }
}

/// Opciones de ajuste que un preset puede elegir: pendientes, sueltas y que no dependen del gusto de cada uno.
fn eligible(i: &Item) -> bool {
    !i.header && !i.done && i.group == 0 && !i.id.starts_with("startup:") && i.id != "game"
}

/// Las opciones que marca el preset, de las listas de Privacidad, Rendimiento y Red.
pub fn select(preset: Preset, privacy: &[Item], performance: &[Item], network: &[Item]) -> Vec<Item> {
    let undoable = |i: &Item| i.steps.iter().all(|s| s.action.undoable());
    let safe = |i: &&Item| eligible(i) && i.risk == Risk::Safe;
    let mut out: Vec<Item> = privacy.iter().filter(safe).cloned().collect();
    if preset != Preset::Minimal {
        out.extend(performance.iter().chain(network).filter(safe).cloned());
    }
    if preset == Preset::Advanced {
        out.extend(performance.iter().filter(|i| eligible(i) && i.risk == Risk::Review && undoable(i)).cloned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{actions::Action, plan::Step, tasks::Task};

    fn item(id: &str, risk: Risk, undoable: bool) -> Item {
        let action = if undoable { Action::SetPowerPlan { new: "a".into(), prev: "b".into() } } else { Action::Maintenance { task: Task::ResetNetwork } };
        Item::new(id, id, "", risk, vec![Step { action }])
    }

    #[test]
    fn each_level_adds_to_the_previous_and_skips_personal_choices() {
        let privacy = vec![item("p-safe", Risk::Safe, true)];
        let mut grouped = item("plan", Risk::Safe, true);
        grouped.group = 1;
        let mut done = item("done", Risk::Safe, true);
        done.done = true;
        let perf = vec![item("svc", Risk::Safe, true), item("fx", Risk::Review, true), item("startup:x", Risk::Safe, true), item("game", Risk::Safe, true), grouped, done];
        let net = vec![item("flush", Risk::Safe, false), item("reset", Risk::Review, false)];
        let ids = |p| select(p, &privacy, &perf, &net).into_iter().map(|i| i.id).collect::<Vec<_>>();
        assert_eq!(ids(Preset::Minimal), ["p-safe"]);
        assert_eq!(ids(Preset::Recommended), ["p-safe", "svc", "flush"]);
        assert_eq!(ids(Preset::Advanced), ["p-safe", "svc", "flush", "fx"]);
    }
}
