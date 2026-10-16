//! Opciones de cada sección y los pasos que las aplican.
//! Cada sección arma una lista de `Item`: lo que se muestra (título, explicación, riesgo) y lo que se haría
//! (`steps`, acciones del journal con el valor de antes ya leído). Nada se aplica sin pasar por `apply`.
use crate::actions::{Action, Journal};

/// Verde (seguro) o amarillo (revisar antes). Lo amarillo viene desmarcado.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Risk {
    Safe,
    Review,
}

/// Una acción concreta, ya armada para poder mostrarla y revertirla.
#[derive(Clone, Debug)]
pub struct Step {
    pub action: Action,
}

impl Step {
    /// Qué va a pasar, con los valores exactos.
    pub fn preview(&self) -> String {
        self.action.describe()
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub desc: String,
    pub risk: Risk,
    /// 0 = casilla suelta; otro número = grupo donde se elige una sola opción.
    pub group: u8,
    /// Ya está aplicado: se muestra, pero no se vuelve a aplicar (deshacer está en Historial).
    pub done: bool,
    /// Se muestra solo detrás de "Más opciones".
    pub more: bool,
    /// Título de grupo en vez de opción.
    pub header: bool,
    pub steps: Vec<Step>,
}

impl Item {
    pub fn new(id: &str, title: &str, desc: &str, risk: Risk, steps: Vec<Step>) -> Self {
        Self { id: id.into(), title: title.into(), desc: desc.into(), risk, group: 0, done: false, more: false, header: false, steps }
    }

    pub fn header(title: &str) -> Self {
        Self { header: true, ..Self::new("", title, "", Risk::Safe, vec![]) }
    }

    /// Marcada de entrada: solo lo verde que todavía no está aplicado.
    pub fn default_checked(&self) -> bool {
        !self.header && !self.done && self.group == 0 && self.risk == Risk::Safe
    }
}

/// Resultado de aplicar: cuántas opciones se aplicaron y cuáles fallaron (con el motivo).
#[derive(Default, Debug)]
pub struct Report {
    pub applied: usize,
    pub failed: Vec<(String, String)>,
}

/// Aplica las opciones elegidas, una por una, registrando cada paso en el journal.
/// Si un paso falla, esa opción se corta ahí y se sigue con la siguiente.
pub fn apply(journal: &mut Journal, items: &[&Item]) -> Report {
    let mut report = Report::default();
    for item in items {
        let mut ok = true;
        for step in &item.steps {
            if let Err(e) = journal.apply(&item.title, step.action.clone()) {
                report.failed.push((item.title.clone(), e.to_string()));
                ok = false;
                break;
            }
        }
        report.applied += ok as usize;
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Hive;

    #[test]
    fn only_safe_pending_items_start_checked() {
        let safe = Item::new("a", "A", "", Risk::Safe, vec![]);
        let review = Item::new("b", "B", "", Risk::Review, vec![]);
        let done = Item { done: true, ..safe.clone() };
        let radio = Item { group: 1, ..safe.clone() };
        assert!(safe.default_checked());
        assert!(!review.default_checked() && !done.default_checked() && !radio.default_checked());
        assert!(!Item::header("x").default_checked());
    }

    #[test]
    fn preview_shows_before_and_after() {
        let step = Step { action: Action::SetDword { hive: Hive::User, path: r"Software\X".into(), name: "V".into(), new: 0, prev: Some(1) } };
        assert_eq!(step.preview(), r"HKCU\Software\X\V: 1 → 0");
    }
}
