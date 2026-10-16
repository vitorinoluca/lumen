//! Pantallas con opciones (Privacidad, Rendimiento, ...) e Historial.
//! Cada sección devuelve una lista de `Item` (ver `lumen_core::plan`); acá se muestra, se marca,
//! se enseña la vista previa y se aplica con punto de restauración y registro en el journal.
use crate::{exe_icon, fetch_icons, ExeIcons, HistRow, Main, OptionRow, PreviewRow};
use lumen_core::{
    actions::Journal,
    plan::{self, Item},
    disk, network, performance, presets::{self, Preset}, privacy, repair, restore, startup, updates,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::{Cell, RefCell}, rc::Rc, sync::{Arc, Mutex}, thread};

/// Primera y última página que usan la pantalla genérica de opciones; 15 es el Historial.
/// Qué llamar al abrir una página, y cómo volver a dibujar la lista (cuando llegan íconos).
pub type OpenPage = Rc<dyn Fn(i32)>;
pub type Redraw = Rc<dyn Fn()>;

pub const FIRST: i32 = 10;
pub const LAST: i32 = 15;
const HISTORY: i32 = 15;

/// Alto de una opción y de un título de grupo (tiene que coincidir con main.slint).
const ROW_H: f32 = 66.0;
const HEADER_H: f32 = 44.0;

/// Lo que reunió el análisis de Inicio: espacio liberable, apps de inicio y las opciones de cada sección.
struct Home {
    free: u64,
    startup: usize,
    privacy: Vec<Item>,
    performance: Vec<Item>,
    network: Vec<Item>,
}

/// Opciones leídas por un hilo: (sección, opciones). Vacío mientras no llegó nada.
type Loaded = Option<(i32, Vec<Item>)>;

struct State {
    section: Cell<i32>,
    items: RefCell<Vec<Item>>,
    /// Marcada o no, en el mismo orden que `items`.
    checked: RefCell<Vec<bool>>,
    /// El punto de restauración se pide una vez por sesión: Windows no deja crear más de uno cada 24 h.
    restore_tried: Cell<bool>,
    journal: Rc<RefCell<Journal>>,
    /// Apps preinstaladas de este usuario: se leen en un hilo al abrir lumen porque Windows tarda un par de segundos.
    apps: Arc<Mutex<Vec<String>>>,
    icons: ExeIcons,
    /// Opciones que llegaron del hilo que las leyó (sección, opciones).
    loaded: Arc<Mutex<Loaded>>,
    /// Lo que se está aplicando, y lo que pasó (lo deja el hilo que ejecuta).
    running: RefCell<Vec<Item>>,
    runs: Arc<Mutex<(Vec<plan::Run>, String)>>,
    /// Resultado del análisis de Inicio (lo deja el hilo) y lo que la pantalla guarda para armar presets.
    analysed: Arc<Mutex<Option<Home>>>,
    home: RefCell<Option<Home>>,
}

/// Título, explicación y texto del botón principal de cada sección.
/// Marca en la pantalla lo que elige el preset y lo deja listo para la vista previa.
fn select_preset(ui: &Main, st: &State, index: i32) {
    let home = st.home.borrow();
    let Some(h) = home.as_ref() else { return };
    let preset = Preset::from_index(index);
    let items = presets::select(preset, &h.privacy, &h.performance, &h.network);
    ui.set_home_count(items.len() as i32);
    ui.set_home_desc(
        match preset {
            Preset::Minimal => "Solo privacidad: menos publicidad y menos datos enviados a Microsoft. Es lo más suave.",
            Preset::Recommended => "Privacidad, servicios que casi nadie usa y la caché de DNS. Todo lo que está en verde.",
            Preset::Advanced => "Lo recomendado más efectos visuales, segundo plano y servicios que dependen de tu uso. Se puede deshacer, pero cambia cómo se ve y se comporta la PC.",
        }
        .into(),
    );
    *st.checked.borrow_mut() = vec![true; items.len()];
    *st.items.borrow_mut() = items;
    st.section.set(0);
    ui.set_sc_button("Optimizar".into());
}

/// Termina el análisis de Inicio: muestra las cifras y deja marcado el preset elegido.
fn analysed(ui: &Main, st: &State) {
    let Some(h) = st.analysed.lock().unwrap().take() else { return };
    ui.set_home_free(crate::size(h.free).into());
    ui.set_home_startup(h.startup as i32);
    *st.home.borrow_mut() = Some(h);
    select_preset(ui, st, ui.get_home_preset());
    ui.set_home_state(2);
}

fn meta(section: i32) -> (&'static str, &'static str, &'static str) {
    match section {
        0 => ("Inicio", "", "Optimizar"),
        10 => ("Rendimiento", "Que la PC arranque y responda más rápido: inicio, servicios, energía, efectos y segundo plano.", "Optimizar"),
        12 => ("Red", "Arreglá problemas de conexión y elegí qué servidor DNS usar.", "Optimizar"),
        13 => ("Reparación", "Revisá y reparás Windows cuando algo anda mal. Son tareas largas: no cierres lumen mientras corren.", "Reparar"),
        14 => ("Actualizaciones", "Elegí cómo se actualiza Windows.", "Aplicar"),
        11 => ("Privacidad", "Menos datos enviados a Microsoft y menos publicidad en Windows.", "Optimizar"),
        _ => ("", "", "Optimizar"),
    }
}

fn items_for(section: i32, apps: &Mutex<Vec<String>>) -> Vec<Item> {
    match section {
        10 => performance::items(),
        12 => network::items(),
        13 => repair::items(),
        14 => updates::items(),
        11 => [privacy::items(), privacy::bloat_items(&apps.lock().unwrap())].concat(),
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
        let (icon, has_icon) = exe_icon(&st.icons, &it.icon);
        rows.push(OptionRow {
            icon,
            has_icon,
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

/// Muestra la sección ya: las opciones se leen en un hilo (algunas consultan a Windows y tardan) y llegan por `sc-loaded`.
fn load(ui: &Main, st: &State, section: i32) {
    let (title, sub, button) = meta(section);
    ui.set_sc_title(title.into());
    ui.set_sc_sub(sub.into());
    ui.set_sc_button(button.into());
    st.section.set(section);
    ui.set_sc_loading(true);
    let (apps, loaded, weak) = (st.apps.clone(), st.loaded.clone(), ui.as_weak());
    thread::spawn(move || {
        *loaded.lock().unwrap() = Some((section, items_for(section, &apps)));
        let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_sc_loaded());
    });
}

/// Las opciones leídas llegaron: si todavía es la sección que se mira, se muestran.
fn loaded(ui: &Main, st: &State) {
    let Some((section, items)) = st.loaded.lock().unwrap().take() else { return };
    if section != st.section.get() {
        return;
    }
    fetch_icons(&st.icons, items.iter().map(|it| it.icon.clone()), ui.as_weak());
    *st.checked.borrow_mut() = items.iter().map(Item::default_checked).collect();
    *st.items.borrow_mut() = items;
    ui.set_sc_loading(false);
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

/// ¿Alguno de estos pasos cambia algo en la PC? Si todo es lectura no hace falta punto de restauración.
fn changes_system(items: &[Item]) -> bool {
    items.iter().flat_map(|it| &it.steps).any(|s| !s.action.is_read_only())
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
            PreviewRow { title: it.title.clone().into(), detail: detail.into(), undoable: it.steps.iter().all(|s| s.action.undoable()) }
        })
        .collect();
    ui.set_sc_preview_rows(ModelRc::new(VecModel::from(rows)));
    ui.set_sc_preview_h(height);
    let picked: Vec<Item> = chosen(st).into_iter().map(|i| items[i].clone()).collect();
    ui.set_sc_preview_note(
        if changes_system(&picked) {
            "Antes de aplicar se crea un punto de restauración de Windows y se guarda el valor original de cada ajuste. Lo que se puede deshacer, se deshace desde Historial."
        } else {
            "Solo se lee información de tu PC: no se cambia nada."
        }
        .into(),
    );
    ui.set_sc_preview(true);
}

/// Terminó de ejecutarse: se anota en el historial y se cuenta el resultado.
fn finish(ui: &Main, st: &State) {
    let items = std::mem::take(&mut *st.running.borrow_mut());
    let (runs, restore_note) = std::mem::take(&mut *st.runs.lock().unwrap());
    let report = plan::record(&mut st.journal.borrow_mut(), &items, &runs);
    // Las apps quitadas ya no figuran en la lista guardada.
    for it in items.iter().filter(|it| !report.failed.iter().any(|(t, _)| *t == it.title)) {
        if let Some(pkg) = it.id.strip_prefix("app:") {
            st.apps.lock().unwrap().retain(|a| a != pkg);
        }
    }
    let mut msg = match report.applied {
        0 if !changes_system(&items) => "No se pudo completar la revisión.".to_string(),
        n if n > 0 && !changes_system(&items) => "Listo: revisión terminada.".to_string(),
        0 => "No se aplicó ningún cambio.".to_string(),
        1 => "Listo: 1 cambio aplicado.".to_string(),
        n => format!("Listo: {n} cambios aplicados."),
    };
    for note in &report.notes {
        msg += &format!(" {}", capitalize(note));
    }
    if let Some((title, why)) = report.failed.first() {
        msg += &format!(" No se pudo \"{title}\": {why}");
    }
    msg += &restore_note;
    if st.section.get() == 0 {
        st.items.borrow_mut().clear();
        ui.set_home_state(3);
    } else {
        load(ui, st, st.section.get());
    }
    ui.set_sc_result(msg.into());
    ui.set_sc_busy(false);
}

/// Primera letra en mayúscula, para que el resumen de una tarea quede como frase.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect::<String>() + ".").unwrap_or_default()
}

fn load_history(ui: &Main, journal: &Journal) {
    let rows: Vec<HistRow> = journal
        .entries
        .iter()
        .enumerate()
        .rev()
        .map(|(id, e)| HistRow { id: id as i32, title: e.description.clone().into(), detail: e.action.describe().into(), applied: e.applied, undoable: e.undoable() })
        .collect();
    ui.set_hist(ModelRc::new(VecModel::from(rows)));
}

/// Conecta las pantallas con opciones y el Historial. Devuelve lo que hay que llamar al abrir una página.
pub fn wire(ui: &Main, journal: Rc<RefCell<Journal>>, icons: ExeIcons) -> (OpenPage, Redraw) {
    let st = Rc::new(State { section: Cell::new(0), items: RefCell::default(), checked: RefCell::default(), restore_tried: Cell::new(false), journal: journal.clone(), apps: Arc::default(), icons, loaded: Arc::default(), running: RefCell::default(), runs: Arc::default(), analysed: Arc::default(), home: RefCell::default() });
    {
        let apps = st.apps.clone();
        thread::spawn(move || *apps.lock().unwrap() = privacy::installed_apps());
    }

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
    // Tras confirmar: punto de restauración (tarda unos segundos) y ejecución, todo en un hilo.
    // El journal vive en la pantalla: lo que pasó vuelve por `sc-finish` y recién ahí se anota.
    ui.on_sc_run({
        let (weak, st) = (ui.as_weak(), st.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let picked: Vec<Item> = {
                let items = st.items.borrow();
                chosen(&st).into_iter().map(|i| items[i].clone()).collect()
            };
            *st.running.borrow_mut() = picked.clone();
            ui.set_sc_busy(true);
            ui.set_sc_result(if changes_system(&picked) { "Creando el punto de restauración, puede tardar unos segundos…" } else { "Revisando…" }.into());
            // El punto de restauración se pide la primera vez que algo cambia, no por leer información.
            let first = changes_system(&picked) && !st.restore_tried.replace(true);
            let section = st.section.get();
            let (weak, runs) = (weak.clone(), st.runs.clone());
            thread::spawn(move || {
                let note = if first {
                    match restore::create(&format!("sección {}", meta(section).0)) {
                        Ok(()) => " Se creó un punto de restauración.".to_string(),
                        Err(e) => format!(" Sin punto de restauración ({e}); los cambios igual se pueden deshacer desde Historial."),
                    }
                } else {
                    String::new()
                };
                let _ = weak.upgrade_in_event_loop(|ui| ui.set_sc_result("Aplicando los cambios, puede tardar…".into()));
                let done = plan::run(&picked);
                *runs.lock().unwrap() = (done, note);
                let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_sc_finish());
            });
        }
    });
    ui.on_sc_finish({
        let (weak, st) = (ui.as_weak(), st.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                finish(&ui, &st);
            }
        }
    });
    ui.on_sc_loaded({
        let (weak, st) = (ui.as_weak(), st.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                loaded(&ui, &st);
            }
        }
    });
    ui.on_home_analyze({
        let (weak, st) = (ui.as_weak(), st.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_home_state(1);
            ui.set_sc_result("".into());
            let (weak, out) = (weak.clone(), st.analysed.clone());
            thread::spawn(move || {
                let free = disk::targets().iter().map(|t| disk::dir_size(&t.path)).sum();
                let startup = startup::list().iter().filter(|e| e.enabled).count();
                *out.lock().unwrap() = Some(Home { free, startup, privacy: privacy::items(), performance: performance::items(), network: network::items() });
                let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_home_ready());
            });
        }
    });
    ui.on_home_ready({
        let (weak, st) = (ui.as_weak(), st.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                analysed(&ui, &st);
            }
        }
    });
    ui.on_home_select({
        let (weak, st) = (ui.as_weak(), st.clone());
        move |i| {
            if let Some(ui) = weak.upgrade() {
                select_preset(&ui, &st, i);
            }
        }
    });
    ui.on_home_apply({
        let (weak, st) = (ui.as_weak(), st.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                preview(&ui, &st);
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

    // Cuando llegan íconos nuevos se vuelve a dibujar la lista sin tocar lo marcado.
    let redraw: Redraw = Rc::new({
        let (weak, st) = (ui.as_weak(), st.clone());
        move || {
            if let Some(ui) = weak.upgrade().filter(|ui| (FIRST..HISTORY).contains(&ui.get_page())) {
                show(&ui, &st);
            }
        }
    });
    let open: OpenPage = Rc::new({
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
    });
    (open, redraw)
}
