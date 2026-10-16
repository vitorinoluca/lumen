//! Opciones de la sección Reparación: SFC, DISM, reparar Windows Update y salud del disco (SMART).
//! Son tareas largas y casi ninguna se puede deshacer: vienen desmarcadas, salvo la lectura de salud.
use crate::{
    actions::Action,
    plan::{Item, Risk, Step},
    tasks::Task,
};

fn task(id: &str, title: &str, desc: &str, risk: Risk, task: Task, unchecked: bool) -> Item {
    Item { unchecked, ..Item::new(id, title, desc, risk, vec![Step { action: Action::Maintenance { task } }]) }
}

pub fn items() -> Vec<Item> {
    vec![
        task("repair:disk", "Ver la salud del disco (SMART)", "Lee el estado, la temperatura y el desgaste de tus discos. No cambia nada.", Risk::Safe, Task::DiskHealth, false),
        task("repair:sfc", "Reparar los archivos del sistema (SFC)", "Busca archivos de Windows dañados y los reemplaza. Tarda varios minutos.", Risk::Safe, Task::Sfc, true),
        task("repair:dism", "Reparar la imagen de Windows (DISM)", "Arregla la base de componentes de Windows. Tarda varios minutos y necesita internet.", Risk::Safe, Task::Dism, true),
        task("repair:update", "Reparar Windows Update", "Reinicia las descargas de actualizaciones cuando se quedan trabadas. No se puede deshacer.", Risk::Review, Task::RepairUpdate, true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_repairs_start_unchecked_and_only_the_health_check_is_marked() {
        let items = items();
        let marked: Vec<_> = items.iter().filter(|i| i.default_checked()).map(|i| i.id.as_str()).collect();
        assert_eq!(marked, ["repair:disk"]);
    }
}
