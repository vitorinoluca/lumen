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
    /// Seguro pero largo o poco habitual: arranca desmarcado igual.
    pub unchecked: bool,
    /// Programa cuyo ícono se muestra al lado (ruta del .exe); vacío = sin ícono.
    pub icon: String,
    pub steps: Vec<Step>,
}

impl Item {
    pub fn new(id: &str, title: &str, desc: &str, risk: Risk, steps: Vec<Step>) -> Self {
        Self { id: id.into(), title: title.into(), desc: desc.into(), risk, group: 0, done: false, more: false, header: false, unchecked: false, icon: String::new(), steps }
    }

    pub fn header(title: &str) -> Self {
        Self { header: true, ..Self::new("", title, "", Risk::Safe, vec![]) }
    }

    /// Marcada de entrada: solo lo verde que todavía no está aplicado.
    pub fn default_checked(&self) -> bool {
        !self.header && !self.done && !self.unchecked && self.group == 0 && self.risk == Risk::Safe
    }
}

/// Resultado de aplicar: cuántas opciones se aplicaron y cuáles fallaron (con el motivo).
#[derive(Default, Debug)]
pub struct Report {
    pub applied: usize,
    pub failed: Vec<(String, String)>,
    /// Resúmenes de las tareas que los dejaron.
    pub notes: Vec<String>,
}

/// Lo que pasó al ejecutar una opción: cuántos pasos se hicieron y, si uno falló, por qué.
#[derive(Debug, Clone, Default)]
pub struct Run {
    pub done: usize,
    pub error: Option<String>,
    /// Resúmenes que dejaron las tareas (ej. la salud del disco).
    pub notes: Vec<String>,
}

/// Ejecuta las opciones elegidas sin tocar el journal. Puede tardar (SFC, DISM...): va en un hilo.
/// Si un paso falla, esa opción se corta ahí y se sigue con la siguiente.
pub fn run(items: &[Item]) -> Vec<Run> {
    items
        .iter()
        .map(|item| {
            let mut run = Run::default();
            for step in &item.steps {
                match step.action.execute() {
                    Ok(note) => {
                        run.done += 1;
                        run.notes.extend(note);
                    }
                    Err(e) => {
                        run.error = Some(e.to_string());
                        break;
                    }
                }
            }
            run
        })
        .collect()
}

/// Anota en el journal lo que `run` hizo (solo los pasos que salieron bien) y arma el resumen.
pub fn record(journal: &mut Journal, items: &[Item], runs: &[Run]) -> Report {
    let mut report = Report::default();
    for (item, run) in items.iter().zip(runs) {
        report.notes.extend(run.notes.iter().cloned());
        for step in item.steps[..run.done].iter().filter(|s| !s.action.is_read_only()) {
            if let Err(e) = journal.record(&item.title, step.action.clone()) {
                report.failed.push((item.title.clone(), format!("no se pudo anotar en el historial: {e}")));
            }
        }
        match &run.error {
            Some(e) => report.failed.push((item.title.clone(), e.clone())),
            None => report.applied += 1,
        }
    }
    report
}

/// Ejecuta y anota de una vez (para cuando no hace falta un hilo).
pub fn apply(journal: &mut Journal, items: &[&Item]) -> Report {
    let owned: Vec<Item> = items.iter().map(|i| (*i).clone()).collect();
    record(journal, &owned, &run(&owned))
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
    fn apply_counts_successes_and_reports_failures_without_stopping() {
        let dir = std::env::temp_dir().join(format!("lumen-plan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        std::fs::write(&a, "x").unwrap();
        let mv = |from: &std::path::Path, to: &std::path::Path| vec![Step { action: Action::MoveFile { from: from.into(), to: to.into() } }];
        let ok = Item::new("ok", "Mover a", "", Risk::Safe, mv(&a, &b));
        let bad = Item::new("bad", "Mover inexistente", "", Risk::Safe, mv(&dir.join("no-existe"), &dir.join("c.txt")));
        let mut journal = Journal::open(dir.join("journal.json")).unwrap();
        let report = apply(&mut journal, &[&bad, &ok]);
        assert_eq!(report.applied, 1);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].0, "Mover inexistente");
        assert!(b.exists() && !a.exists());
        assert_eq!(journal.entries.len(), 1); // solo lo que se aplicó queda en el historial
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preview_shows_before_and_after() {
        let step = Step { action: Action::SetDword { hive: Hive::User, path: r"Software\X".into(), name: "V".into(), new: 0, prev: Some(1) } };
        assert_eq!(step.preview(), r"HKCU\Software\X\V: 1 → 0");
    }
}
