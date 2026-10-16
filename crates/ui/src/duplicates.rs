//! Pestaña "Duplicados" de Limpieza: busca en un hilo, muestra los grupos y mueve las copias a la reserva.
use crate::{DupRow, Main};
use lumen_core::{
    actions::Journal,
    disk,
    duplicates::{self, Group},
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex}, thread};

/// Duplicados de menos de 1 MB no valen la pena: el espacio que liberan no se nota.
const MIN_SIZE: u64 = 1 << 20;
/// Cuántos grupos se muestran: los que más espacio liberan.
const MAX_GROUPS: usize = 100;

fn size(b: u64) -> String {
    crate::size(b)
}

/// Conecta la pestaña. `journal` registra el movimiento para poder deshacerlo.
pub fn wire(ui: &Main, journal: Rc<RefCell<Journal>>) {
    // Lo que encuentra el hilo, para que lo recoja la pantalla; y lo que se muestra ahora.
    let found: Arc<Mutex<Vec<Group>>> = Arc::default();
    let shown: Rc<RefCell<Vec<Group>>> = Rc::default();
    let ids: Rc<RefCell<Vec<usize>>> = Rc::default();

    ui.on_dup_scan({
        let (weak, found) = (ui.as_weak(), found.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_dup_phase(1);
            let (weak, found) = (weak.clone(), found.clone());
            thread::spawn(move || {
                let mut groups = duplicates::find(&duplicates::default_roots(), MIN_SIZE);
                groups.truncate(MAX_GROUPS);
                *found.lock().unwrap() = groups;
                let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_dup_found());
            });
        }
    });
    ui.on_dup_found({
        let (weak, found, shown) = (ui.as_weak(), found.clone(), shown.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let groups = std::mem::take(&mut *found.lock().unwrap());
            let total: u64 = groups.iter().map(Group::wasted).sum();
            let rows: Vec<DupRow> = groups
                .iter()
                .map(|g| DupRow {
                    name: g.files[0].file_name().unwrap_or_default().to_string_lossy().into_owned().into(),
                    folder: g.files[0].parent().and_then(|p| p.file_name()).unwrap_or_default().to_string_lossy().into_owned().into(),
                    copies: g.files.len() as i32 - 1,
                    wasted: size(g.wasted()).into(),
                })
                .collect();
            ui.set_dup_total(size(total).into());
            ui.set_dup_rows(ModelRc::new(VecModel::from(rows)));
            *shown.borrow_mut() = groups;
            ui.set_dup_phase(2);
        }
    });
    ui.on_dup_clean({
        let (weak, journal, shown, ids) = (ui.as_weak(), journal.clone(), shown.clone(), ids.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            // De cada grupo se conserva el primero (el más antiguo) y el resto pasa a la reserva.
            let copies: Vec<_> = shown.borrow().iter().flat_map(|g| g.files[1..].to_vec()).collect();
            ids.borrow_mut().clear();
            ui.set_dup_can_undo(false);
            let cleaned = match disk::stash_files(&mut journal.borrow_mut(), "Quitar archivos duplicados", &copies, &disk::stash_root()) {
                Ok(c) => c,
                Err(e) => {
                    ui.set_dup_result(format!("No se pudo mover las copias: {e}").into());
                    ui.set_dup_phase(3);
                    return;
                }
            };
            ids.borrow_mut().extend(cleaned.id);
            let (bytes, moved, skipped) = (cleaned.bytes, cleaned.moved, cleaned.skipped);
            let mut msg = if moved == 0 { "No se pudo mover ninguna copia: están en uso.".to_string() } else { format!("Listo: liberaste {} ({moved} copias en la reserva).", size(bytes)) };
            if skipped > 0 && moved > 0 {
                msg += &format!(" {skipped} quedaron porque estaban en uso.");
            }
            ui.set_dup_can_undo(moved > 0);
            ui.set_dup_result(msg.into());
            ui.set_dup_phase(3);
        }
    });
    ui.on_dup_undo({
        let (weak, journal, ids) = (ui.as_weak(), journal.clone(), ids.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let failed = ids.borrow().iter().rev().filter(|&&id| journal.borrow_mut().revert(id).is_err()).count();
            ids.borrow_mut().clear();
            ui.set_dup_rows(ModelRc::new(VecModel::from(Vec::<DupRow>::new())));
            ui.set_dup_result(if failed == 0 { "Copias devueltas a su lugar.".into() } else { format!("{failed} copias no se pudieron devolver.").into() });
            ui.set_dup_can_undo(false);
            ui.set_dup_phase(3);
        }
    });
    ui.on_dup_reset({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_dup_rows(ModelRc::new(VecModel::from(Vec::<DupRow>::new())));
                ui.set_dup_phase(0);
            }
        }
    });
}
