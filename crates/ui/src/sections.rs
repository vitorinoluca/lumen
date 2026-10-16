//! Pantallas con opciones (Privacidad, Rendimiento, ...) e Historial.
//! Cada sección devuelve una lista de `Item` (ver `lumen_core::plan`); acá se muestra, se marca,
//! se enseña la vista previa y se aplica con punto de restauración y registro en el journal.
use crate::{HistRow, Main, OptionRow, PreviewRow};
use lumen_core::{
    actions::Journal,
    plan::{self, Item},
    privacy, restore,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::{Cell, RefCell}, rc::Rc, thread};

/// Primera y última página que usan la pantalla genérica de opciones; 15 es el Historial.
pub const FIRST: i32 = 10;
pub const LAST: i32 = 15;
const HISTORY: i32 = 15;

/// Alto de una opción y de un título de grupo (tiene que coincidir con main.slint).
const ROW_H: f32 = 66.0;
const HEADER_H: f32 = 44.0;

struct State {
    section: Cell<i32>,
    items: RefCell<Vec<Item>>,
    /// Marcada o no, en el mismo orden que `items`.
    checked: RefCell<Vec<bool>>,
    /// El punto de restauración se pide una vez por sesión: Windows no deja crear más de uno cada 24 h.
    restore_tried: Cell<bool>,
    journal: Rc<RefCell<Journal>>,
}

/// Título, explicación y texto del botón principal de cada sección.
fn meta(section: i32) -> (&'static str, &'static str, &'static str) {
    match section {
        11 => ("Privacidad", "Menos datos enviados a Microsoft y menos publicidad en Windows.", "Optimizar"),
        _ => ("", "", "Optimizar"),
    }
}

fn items_for(section: i32) -> Vec<Item> {
    match section {
        11 => privacy::items(),
        _ => vec![],
    }
}

/// Arma la lista que se ve: respeta "Más opciones" y no deja títulos de grupo vacíos.
fn show(ui: &Main, st: &State) {
    let items = st.items.borrow();
    let checked = st.checked.borrow();
    let more = ui.get_sc_more();
    let mut rows: Vec<OptionRow> = vec![];
    for (i, it) in items.iter().enumerate() {
        if it.more && !more {
            continue;
        }
        rows.push(OptionRow {
            idx: i as i32,
            title: it.title.clone().into(),
            desc: it.desc.clone().into(),
            risk: (it.risk == plan::Risk::Review) as i32,
            checked: checked[i],
            done: it.done,
            group: it.group as i32,
            header: it.header,
        });
    }
    // Un título sin opciones debajo sobra.
    let mut clean: Vec<OptionRow> = vec![];
    for (n, r) in rows.iter().enumerate() {
        if !r.header || rows.get(n + 1).is_some_and(|next| !next.header) {
            clean.push(r.clone());
        }
    }
    let height: f32 = clean.iter().map(|r| if r.header { HEADER_H } else { ROW_H }).sum();
    ui.set_sc_h(height);
    ui.set_sc_rows(ModelRc::new(VecModel::from(clean)));
    ui.set_sc_selected(checked.iter().zip(items.iter()).filter(|(c, it)| **c && !it.done).count() as i32);
    ui.set_sc_more_count(items.iter().filter(|it| it.more && !it.header).count() as i32);
}

fn load(ui: &Main, st: &State, section: i32) {
    let (title, sub, button) = meta(section);
    ui.set_sc_title(title.into());
    ui.set_sc_sub(sub.into());
    ui.set_sc_button(button.into());
    let items = items_for(section);
    *st.checked.borrow_mut() = items.iter().map(Item::default_checked).collect();
    *st.items.borrow_mut() = items;
    st.section.set(section);
    show(ui, st);
}

fn toggle(ui: &Main, st: &State, idx: usize) {
    let items = st.items.borrow();
    let Some(item) = items.get(idx).filter(|it| !it.done && !it.header) else { return };
    let mut checked = st.checked.borrow_mut();
    let now = !checked[idx];
    // En un grupo se elige una sola; marcar una desmarca las demás.
    if item.group > 0 && now {
        for (j, it) in items.iter().enumerate() {
            if it.group == item.group {
                checked[j] = false;
            }
        }
    }
    checked[idx] = now;
    drop((items, checked));
    show(ui, st);
}

/// Índices de lo marcado que se puede aplicar.
fn chosen(st: &State) -> Vec<usize> {
    let checked = st.checked.borrow();
    st.items.borrow().iter().enumerate().filter(|(i, it)| checked[*i] && !it.done && !it.header).map(|(i, _)| i).collect()
}

/// Muestra qué se va a hacer, con los valores exactos, antes de aplicar nada.
fn preview(ui: &Main, st: &State) {
    let items = st.items.borrow();
    let mut height = 0.0;
    let rows: Vec<PreviewRow> = chosen(st)
        .into_iter()
        .map(|i| {
            let it = &items[i];
            let detail = it.steps.iter().map(|s| s.preview()).collect::<Vec<_>>().join("\n");
            // Estimación del alto: título, una línea cada ~70 caracteres por paso y el aire de abajo.
            height += 22.0 + detail.lines().map(|l| (l.chars().count() / 70 + 1) as f32 * 16.0).sum::<f32>() + 12.0;
            PreviewRow { title: it.title.clone().into(), detail: detail.into(), undoable: true }
        })
        .collect();
    ui.set_sc_preview_rows(ModelRc::new(VecModel::from(rows)));
    ui.set_sc_preview_h(height);
    ui.set_sc_preview_note(
        "Antes de aplicar se crea un punto de restauración de Windows y se guarda el valor original de cada ajuste. Todo se puede deshacer desde Historial.".into(),
    );
    ui.set_sc_preview(true);
}

/// Aplica lo marcado y cuenta el resultado.
fn finish(ui: &Main, st: &State, restore_note: &str) {
    let items = st.items.borrow().clone();
    let picked: Vec<&Item> = chosen(st).into_iter().map(|i| &items[i]).collect();
    let report = plan::apply(&mut st.journal.borrow_mut(), &picked);
    let mut msg = match report.applied {
        0 => "No se aplicó ningún cambio.".to_string(),
        1 => "Listo: 1 cambio aplicado.".to_string(),
        n => format!("Listo: {n} cambios aplicados."),
    };
    if let Some((title, why)) = report.failed.first() {
        msg += &format!(" No se pudo \"{title}\": {why}");
    }
    msg += restore_note;
    load(ui, st, st.section.get());
    ui.set_sc_result(msg.into());
    ui.set_sc_busy(false);
}

fn load_history(ui: &Main, journal: &Journal) {
    let rows: Vec<HistRow> = journal
        .entries
        .iter()
        .enumerate()
        .rev()
        .map(|(id, e)| HistRow { id: id as i32, title: e.description.clone().into(), detail: e.action.describe().into(), applied: e.applied, undoable: true })
        .collect();
    ui.set_hist(ModelRc::new(VecModel::from(rows)));
}

/// Conecta las pantallas con opciones y el Historial. Devuelve lo que hay que llamar al abrir una página.
pub fn wire(ui: &Main, journal: Rc<RefCell<Journal>>) -> Rc<dyn Fn(i32)> {
    let st = Rc::new(State { section: Cell::new(0), items: RefCell::default(), checked: RefCell::default(), restore_tried: Cell::new(false), journal: journal.clone() });

    ui.on_sc_toggle({
        let (weak, st) = (ui.as_weak(), st.clone());
        move |i| {
            if let Some(ui) = weak.upgrade() {
                toggle(&ui, &st, i as usize);
            }
        }
    });
    ui.on_sc_reload({
        let (weak, st) = (ui.as_weak(), st.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                show(&ui, &st);
            }
        }
    });
    ui.on_sc_apply({
        let (weak, st) = (ui.as_weak(), st.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                preview(&ui, &st);
            }
        }
    });
    // Tras confirmar: el punto de restauración tarda unos segundos, va en un hilo; después se aplica acá.
    ui.on_sc_run({
        let (weak, st) = (ui.as_weak(), st.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_sc_busy(true);
            ui.set_sc_result("Creando el punto de restauración, puede tardar unos segundos…".into());
            let first = !st.restore_tried.replace(true);
            let section = st.section.get();
            let weak = weak.clone();
            thread::spawn(move || {
                let note = if first {
                    match restore::create(&format!("sección {}", meta(section).0)) {
                        Ok(()) => " Se creó un punto de restauración.".to_string(),
                        Err(e) => format!(" Sin punto de restauración ({e}); los cambios igual se pueden deshacer desde Historial."),
                    }
                } else {
                    String::new()
                };
                let _ = weak.upgrade_in_event_loop(move |ui| ui.invoke_sc_finish(note.into()));
            });
        }
    });
    ui.on_sc_finish({
        let (weak, st) = (ui.as_weak(), st.clone());
        move |note| {
            if let Some(ui) = weak.upgrade() {
                finish(&ui, &st, note.as_str());
            }
        }
    });
    ui.on_hist_undo({
        let (weak, journal) = (ui.as_weak(), journal.clone());
        move |id| {
            let Some(ui) = weak.upgrade() else { return };
            let title = journal.borrow().entries.get(id as usize).map(|e| e.description.clone()).unwrap_or_default();
            let msg = match journal.borrow_mut().revert(id as usize) {
                Ok(()) => format!("Deshecho: {title}"),
                Err(e) => format!("No se pudo deshacer: {e}"),
            };
            ui.set_hist_status(msg.into());
            load_history(&ui, &journal.borrow());
        }
    });

    Rc::new({
        let (weak, st) = (ui.as_weak(), st.clone());
        move |page| {
            let Some(ui) = weak.upgrade() else { return };
            if page == HISTORY {
                ui.set_hist_status("".into());
                load_history(&ui, &st.journal.borrow());
            } else {
                ui.set_sc_more(false);
                ui.set_sc_result("".into());
                load(&ui, &st, page);
            }
        }
    })
}
