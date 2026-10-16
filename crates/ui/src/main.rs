#![windows_subsystem = "windows"]
use lumen_core::{
    actions::{Action, Hive, Journal},
    monitor::{Monitor, Snapshot},
    processes::{Processes, SortBy},
    startup::{self, Source},
    disk,
    defender, hardware, hashdb, icons, security, power, privacy, profiles, restore, services,
    uninstall::{self, Leftover},
};
use slint::{ModelRc, Timer, TimerMode, VecModel};
use std::{cell::{Cell, RefCell}, collections::HashMap, env, path::PathBuf, sync::{Arc, Mutex}, rc::Rc, thread, time::Duration};

slint::include_modules!();

/// Íconos ya extraídos por programa: ancho, alto y píxeles RGBA.
type IconCache = Arc<Mutex<HashMap<String, (u32, u32, Vec<u8>)>>>;

fn gb(b: u64) -> String {
    // Base 1024, como muestra Windows (34,2 GB en base 10 son 31,9 GB para el sistema).
    format!("{:.1} GB", b as f64 / 1_073_741_824.0).replace('.', ",")
}

/// "Encendida hace 1 d 11 h" a partir de los segundos de actividad.
fn uptime(secs: u64) -> String {
    let (d, h, m) = (secs / 86400, secs % 86400 / 3600, secs % 3600 / 60);
    if d > 0 { format!("Encendida hace {d} d {h} h") } else { format!("Encendida hace {h} h {m} min") }
}

/// Datos fijos del equipo para el panel; se leen en un hilo al abrir.
fn load_hardware(weak: slint::Weak<Main>) {
    thread::spawn(move || {
        let h = hardware::read();
        let build = h.os_build.split(['(', ')']).nth(1).unwrap_or("").to_string();
        let hw = (
            h.os.clone(),
            format!("64 bits · compilación {build}"),
            h.cpu.clone(),
            format!("{} núcleos / {} hilos · {} MHz", h.cores, h.threads, h.mhz),
            format!("{} de RAM", gb(h.ram_total)),
            if h.gpu.is_empty() { "Gráficos".to_string() } else { h.gpu.clone() },
            if h.vram > 0 { format!("{} de memoria", gb(h.vram)) } else { String::new() },
            h.board.clone(),
            uptime(h.uptime_secs),
        );
        let disks: Vec<(String, String, f32)> = h
            .disks
            .iter()
            .map(|d| (format!("({}) {} · {}", d.mount.trim_end_matches('\\'), d.kind, d.fs), format!("{} usados de {}", gb(d.used), gb(d.total)), d.used as f32 / d.total.max(1) as f32))
            .collect();
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let (os, os_sub, cpu, cpu_sub, ram, gpu, gpu_sub, board, board_sub) = hw;
            ui.set_hw(Hw { os: os.into(), os_sub: os_sub.into(), cpu: cpu.into(), cpu_sub: cpu_sub.into(), ram: ram.into(), gpu: gpu.into(), gpu_sub: gpu_sub.into(), board: board.into(), board_sub: board_sub.into() });
            let rows: Vec<DiskCard> = disks.into_iter().map(|(n, s, f)| DiskCard { name: n.into(), sub: s.into(), fraction: f }).collect();
            ui.set_disks(ModelRc::new(VecModel::from(rows)));
        });
    });
}

fn service_msg(res: std::io::Result<()>, warn: &str) -> String {
    match res {
        Ok(()) => warn.to_string(),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => "Necesitás abrir lumen como administrador para cambiar servicios.".into(),
        Err(e) => format!("No se pudo: {e}"),
    }
}

/// Actualiza el modelo fila por fila sin recrearlo: las filas que ya existen conservan su estado
/// (hover, botones) y la lista no salta al principio. Solo se agregan o quitan las que sobran o faltan.
fn sync_rows<T: Clone + 'static>(model: &VecModel<T>, rows: Vec<T>) {
    use slint::Model;
    let keep = model.row_count().min(rows.len());
    while model.row_count() > rows.len() {
        model.remove(model.row_count() - 1);
    }
    for (i, r) in rows.into_iter().enumerate() {
        if i < keep {
            model.set_row_data(i, r);
        } else {
            model.push(r);
        }
    }
}

/// "2026-10-15T11:46:03.0000000-03:00" -> "15/10/2026 11:46". Vacío = "nunca".
fn fmt_date(iso: &str) -> String {
    match (iso.get(0..4), iso.get(5..7), iso.get(8..10), iso.get(11..16)) {
        (Some(y), Some(m), Some(d), Some(t)) => format!("{d}/{m}/{y} {t}"),
        _ => "nunca".to_string(),
    }
}

fn severity(n: u8) -> &'static str {
    match n {
        1 => "baja",
        2 => "moderada",
        4 => "alta",
        5 => "grave",
        _ => "sin dato",
    }
}

const CONSULTING: &str = "Consultando a Windows Defender…";

/// Consulta a Defender en un hilo y arma las tarjetas. Nunca dice que la PC esté limpia.
fn load_security(weak: slint::Weak<Main>) {
    if let Some(ui) = weak.upgrade() {
        // Solo si no hay otro aviso en pantalla (las notas del análisis no se pisan).
        if ui.get_sec_status().is_empty() {
            ui.set_sec_status(CONSULTING.into());
        }
    }
    thread::spawn(move || {
        let st = defender::status();
        let th = defender::threats();
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let card = |icon: &str, title: &str, sub: String| SecRow { icon: icon.into(), title: title.into(), sub: sub.into(), alert: false };
            let mut cards = vec![];
            let mut note = String::new();
            match st {
                Ok(s) => {
                    let rt = match (s.realtime, s.mode.as_str()) {
                        (_, "Passive") => "En segundo plano: hay otro antivirus como protección principal".to_string(),
                        (true, _) => "Activa".to_string(),
                        (false, _) => "Apagada".to_string(),
                    };
                    let last = if s.quick_scan.is_empty() { "nunca".to_string() } else { fmt_date(&s.quick_scan) };
                    cards.push(card("shield", "Windows Defender", format!("Protección en tiempo real: {} · último análisis rápido: {last}", rt.to_lowercase())));
                    if !s.other_av.is_empty() {
                        note = format!("También hay otro antivirus instalado: {}.", s.other_av.join(", "));
                    }
                }
                Err(e) => note = format!("No se pudo consultar a Defender: {e}"),
            }
            let threats: Vec<SecRow> = th
                .unwrap_or_default()
                .iter()
                .map(|t| SecRow {
                    icon: "shield".into(),
                    title: if t.name.is_empty() { "Detección sin nombre".to_string() } else { t.name.clone() }.into(),
                    // Dos líneas: la ruta arriba y el resto abajo, para que nada quede cortado.
                    sub: format!(
                        "{}\n{} · gravedad {} · {}",
                        t.file(),
                        fmt_date(&t.time),
                        severity(t.severity),
                        if t.active { "sigue activa: abrí Seguridad de Windows" } else { "ya no figura como activa" }
                    )
                    .into(),
                    alert: t.active || t.severity >= 4,
                })
                .collect();
            ui.set_sec_cards(ModelRc::new(VecModel::from(cards)));
            ui.set_threats(ModelRc::new(VecModel::from(threats)));
            // No pisar el aviso de otra operación (revisión, descarga): solo se escribe si esto trae algo
            // que decir o si lo que había era el propio "Consultando…".
            if !note.is_empty() || ui.get_sec_status() == CONSULTING {
                ui.set_sec_status(note.into());
            }
        });
    });
}

/// Mensaje claro para un error al deshabilitar o restaurar.
fn friendly(e: &std::io::Error) -> String {
    let t = e.to_string().to_lowercase();
    if e.kind() == std::io::ErrorKind::PermissionDenied || t.contains("denegado") || t.contains("access") || t.contains("permission") {
        "Necesitás abrir lumen como administrador para hacer este cambio.".to_string()
    } else {
        format!("No se pudo: {e}")
    }
}

/// Alto aproximado de una tarjeta de hallazgo, para el largo del scroll (el texto se ajusta solo).
fn finding_height(f: &security::Finding) -> f32 {
    let lines = |t: &str| t.lines().map(|l| l.chars().count() / 85 + 1).sum::<usize>() as f32;
    let decoded = f.decoded.as_deref().map_or(0.0, |d| 52.0 + lines(d) * 16.0);
    28.0 + 30.0 + lines(&f.detail) * 16.0 + decoded + 44.0 + 12.0
}

/// Tamaño legible: GB desde 1 GB, MB por debajo.
fn size(b: u64) -> String {
    match b {
        b if b >= 1 << 30 => gb(b),
        b if b > 0 && b < 1 << 20 => "<1 MB".to_string(),
        b => format!("{} MB", b >> 20),
    }
}

fn show(ui: &Main, s: &Snapshot) {
    let frac = |a: u64, b: u64| if b == 0 { 0.0 } else { a as f32 / b as f32 };
    ui.set_cpu(s.cpu / 100.0);
    ui.set_ram(frac(s.ram_used, s.ram_total));
    ui.set_ram_text(format!("{} / {}", gb(s.ram_used), gb(s.ram_total)).into());
    ui.set_disk(frac(s.disk_used, s.disk_total));
    ui.set_disk_text(format!("{} / {}", gb(s.disk_used), gb(s.disk_total)).into());
    ui.set_net_text(format!("{} KB/s bajada, {} KB/s subida", s.net_rx / 1024, s.net_tx / 1024).into());
}

fn main() {
    let ui = Main::new().unwrap();
    load_hardware(ui.as_weak());

    // Monitor: lectura en hilo aparte, la UI nunca se bloquea.
    let weak = ui.as_weak();
    thread::spawn(move || {
        let mut mon = Monitor::new();
        loop {
            thread::sleep(Duration::from_secs(1));
            let snap = mon.sample();
            if weak.upgrade_in_event_loop(move |ui| show(&ui, &snap)).is_err() {
                break; // ventana cerrada
            }
        }
    });

    // Procesos: solo se refresca mientras la página está abierta.
    let procs = Rc::new(std::cell::RefCell::new(Processes::default()));
    let procs_model = Rc::new(VecModel::<Row>::default());
    ui.set_procs(ModelRc::from(procs_model.clone()));
    let sort = Rc::new(Cell::new(SortBy::Cpu));
    let refresh = {
        let (weak, procs, sort, model) = (ui.as_weak(), procs.clone(), sort.clone(), procs_model.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.get_page() != 1 {
                return;
            }
            let rows: Vec<Row> = procs
                .borrow_mut()
                .list(sort.get())
                .into_iter()
                .map(|p| Row {
                    pid: p.pid as i32,
                    name: p.name.into(),
                    cpu: format!("{:.0}%", p.cpu).into(),
                    mem: format!("{} MB", p.mem / 1_048_576).into(),
                    path: p.path.map(|p| p.display().to_string()).unwrap_or_default().into(),
                })
                .collect();
            sync_rows(&model, rows);
        }
    };
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_secs(2), refresh.clone());
    ui.on_sort({
        let (sort, refresh) = (sort.clone(), refresh.clone());
        move |i| {
            sort.set([SortBy::Cpu, SortBy::Mem, SortBy::Name][i as usize]);
            refresh();
        }
    });
    // Inicio: cada cambio pasa por el journal para poder deshacerlo.
    let entries = Rc::new(RefCell::new(startup::list()));
    let journal = {
        let dir = PathBuf::from(env::var_os("APPDATA").unwrap_or_default()).join("lumen");
        Rc::new(RefCell::new(Journal::open(dir.join("journal.json")).expect("journal ilegible")))
    };
    let load_startup = {
        let (weak, entries) = (ui.as_weak(), entries.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            *entries.borrow_mut() = startup::list();
            let rows: Vec<StartupRow> = entries
                .borrow()
                .iter()
                .map(|e| StartupRow {
                    name: e.name.clone().into(),
                    command: e.command.clone().into(),
                    scope: match e.source {
                        Source::RunUser => "usuario",
                        Source::RunMachine => "equipo",
                        Source::FolderUser => "carpeta",
                    }
                    .into(),
                    enabled: e.enabled,
                })
                .collect();
            ui.set_startup(ModelRc::new(VecModel::from(rows)));
        }
    };
    ui.on_toggle_startup({
        let (weak, entries, journal, reload) = (ui.as_weak(), entries.clone(), journal.clone(), load_startup.clone());
        move |i, enable| {
            let Some(ui) = weak.upgrade() else { return };
            let e = entries.borrow()[i as usize].clone();
            let verb = if enable { "Activar" } else { "Desactivar" };
            let action = Action::SetStartup { source: e.source, name: e.name.clone(), enable };
            let res = journal.borrow_mut().apply(&format!("{verb} inicio: {}", e.name), action);
            // Las entradas de "equipo" necesitan administrador.
            ui.set_status(res.err().map(|err| format!("No se pudo: {err}")).unwrap_or_default().into());
            reload();
        }
    });
    // Privacidad: activar = apagar la función de Windows (valor privado); desactivar = revertir.
    let load_privacy = {
        let weak = ui.as_weak();
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let rows: Vec<ServiceRow> = privacy::SETTINGS
                .iter()
                .map(|s| ServiceRow {
                    title: s.name.into(),
                    desc: s.desc.into(),
                    level: "seguro".into(),
                    tag: if s.hive == Hive::Machine { "administrador" } else { "" }.into(),
                    enabled: s.is_off(),
                    locked: false,
                })
                .collect();
            ui.set_privacy(ModelRc::new(VecModel::from(rows)));
        }
    };
    ui.on_toggle_privacy({
        let (weak, journal, reload) = (ui.as_weak(), journal.clone(), load_privacy.clone());
        move |i, on| {
            let Some(ui) = weak.upgrade() else { return };
            let s = &privacy::SETTINGS[i as usize];
            let mut j = journal.borrow_mut();
            let res = if on {
                j.apply(&format!("Privacidad: {}", s.name), s.turn_off()).map(|_| ())
            } else {
                // Revierte el último cambio aplicado a este ajuste.
                let last = j.entries.iter().rposition(|e| {
                    e.applied && matches!(&e.action, Action::SetDword { path, name, .. } if path == s.path && name == s.value)
                });
                match last {
                    Some(id) => j.revert(id),
                    None => Err(std::io::Error::other("no hay un cambio de lumen para revertir")),
                }
            };
            ui.set_status(res.err().map(|e| format!("No se pudo: {e}")).unwrap_or_default().into());
            drop(j);
            reload();
        }
    });

    // Programas: desinstalar corre en un hilo (el instalador del programa puede tardar). Los restos
    // quedan pendientes hasta que el usuario los confirme.
    let all_apps = Rc::new(RefCell::new(Vec::<uninstall::App>::new()));
    let apps = Rc::new(RefCell::new(Vec::<uninstall::App>::new())); // lo que se ve: filtrado y ordenado
    let query = Rc::new(RefCell::new(String::new()));
    let sort = Rc::new(Cell::new((0u8, false))); // (0 nombre, 1 peso, 2 editor; descendente)
    let icons: IconCache = Arc::default();
    let pending: Arc<Mutex<(String, Vec<Leftover>)>> = Arc::default();
    let last_ids = Rc::new(RefCell::new(Vec::<usize>::new()));
    let app_key = |a: &uninstall::App| format!("{}|{}", a.name, a.version);

    // Arma la lista visible a partir de todos los programas, la búsqueda, el orden y los íconos ya cargados.
    let rebuild_apps = {
        let (weak, all, apps, query, sort, icons) = (ui.as_weak(), all_apps.clone(), apps.clone(), query.clone(), sort.clone(), icons.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let q = query.borrow().to_lowercase();
            let mut view: Vec<uninstall::App> =
                all.borrow().iter().filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q) || a.publisher.to_lowercase().contains(&q)).cloned().collect();
            let (key, desc) = sort.get();
            // Lo que no tiene dato (peso o editor vacío) va siempre al final, en cualquier sentido.
            let unknown = |a: &uninstall::App| (key == 1 && a.size_kb == 0) || (key == 2 && a.publisher.is_empty());
            view.sort_by(|a, b| {
                unknown(a).cmp(&unknown(b)).then_with(|| {
                    let o = match key {
                        1 => a.size_kb.cmp(&b.size_kb),
                        2 => a.publisher.to_lowercase().cmp(&b.publisher.to_lowercase()),
                        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    };
                    if desc { o.reverse() } else { o }
                })
            });
            let cache = icons.lock().unwrap();
            let rows: Vec<AppRow> = view
                .iter()
                .map(|a| {
                    let icon = cache.get(&format!("{}|{}", a.name, a.version)).map(|(w, h, px)| {
                        slint::Image::from_rgba8(slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(px, *w, *h))
                    });
                    AppRow {
                        name: a.name.clone().into(),
                        detail: [a.publisher.as_str(), a.version.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ").into(),
                        size: if a.size_kb > 0 { size(a.size_kb * 1024) } else { String::new() }.into(),
                        has_icon: icon.is_some(),
                        icon: icon.unwrap_or_default(),
                        initial: a.name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string()).unwrap_or_default().into(),
                    }
                })
                .collect();
            drop(cache);
            *apps.borrow_mut() = view;
            ui.set_apps(ModelRc::new(VecModel::from(rows)));
        }
    };
    // Lee los programas del registro y saca los íconos que falten en un hilo aparte.
    let load_apps = {
        let (weak, all, icons, rebuild) = (ui.as_weak(), all_apps.clone(), icons.clone(), rebuild_apps.clone());
        move || {
            *all.borrow_mut() = uninstall::list();
            rebuild();
            let todo: Vec<(String, uninstall::App)> = {
                let cache = icons.lock().unwrap();
                all.borrow().iter().map(|a| (app_key(a), a.clone())).filter(|(k, _)| !cache.contains_key(k)).collect()
            };
            if todo.is_empty() {
                return;
            }
            let (weak, icons) = (weak.clone(), icons.clone());
            thread::spawn(move || {
                for (key, app) in todo {
                    if let Some(i) = app.icon_sources().iter().find_map(|s| icons::load(s)) {
                        icons.lock().unwrap().insert(key, (i.width, i.height, i.rgba));
                    }
                }
                let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_apps_rebuild());
            });
        }
    };
    ui.on_apps_reload(load_apps.clone());
    ui.on_apps_rebuild(rebuild_apps.clone());
    ui.on_apps_filter({
        let (query, rebuild) = (query.clone(), rebuild_apps.clone());
        move |t| {
            *query.borrow_mut() = t.to_string();
            rebuild();
        }
    });
    ui.on_apps_sort({
        let (weak, sort, rebuild) = (ui.as_weak(), sort.clone(), rebuild_apps.clone());
        move |k| {
            let k = k as u8;
            // Tocar el mismo criterio invierte el orden; el peso arranca de mayor a menor.
            let desc = if sort.get().0 == k { !sort.get().1 } else { k == 1 };
            sort.set((k, desc));
            if let Some(ui) = weak.upgrade() {
                let name = ["nombre", "peso", "editor"][k as usize];
                ui.set_apps_sort_label(format!("{name}, {}", if desc { "de mayor a menor" } else { "de menor a mayor" }).into());
            }
            rebuild();
        }
    });
    ui.on_uninstall({
        let (weak, apps, pending) = (ui.as_weak(), apps.clone(), pending.clone());
        move |i| {
            let Some(ui) = weak.upgrade() else { return };
            let app = apps.borrow()[i as usize].clone();
            ui.set_status("Creando punto de restauración…".into());
            ui.set_leftovers(ModelRc::new(VecModel::from(Vec::<DiskRow>::new())));
            ui.set_can_undo_leftovers(false);
            let (weak, pending) = (weak.clone(), pending.clone());
            thread::spawn(move || {
                let warn = match restore::create(&format!("desinstalar {}", app.name)) {
                    Ok(()) => String::new(),
                    Err(e) => format!("Sin punto de restauración ({e}). "),
                };
                let msg = format!("{warn}Se abrió el desinstalador de {}. Seguí los pasos de su ventana.", app.name);
                let _ = weak.upgrade_in_event_loop(move |ui| ui.set_status(msg.into()));
                let res = uninstall::run_uninstall(&app);
                let found = uninstall::leftovers(&app);
                let rows: Vec<(String, String)> = found
                    .iter()
                    .map(|l| {
                        let (kind, path) = l.describe();
                        (path, kind.to_string())
                    })
                    .collect();
                *pending.lock().unwrap() = (app.name.clone(), found);
                let done = match (res, rows.len()) {
                    (Err(e), _) => format!("No se pudo abrir el desinstalador: {e}"),
                    (Ok(()), 0) => format!("Listo. No quedaron restos de {}.", app.name),
                    (Ok(()), n) => format!("Terminó el desinstalador. Encontramos {n} restos."),
                };
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    let rows: Vec<DiskRow> = rows.into_iter().map(|(n, s)| DiskRow { name: n.into(), size: s.into() }).collect();
                    ui.set_leftovers(ModelRc::new(VecModel::from(rows)));
                    ui.set_status(done.into());
                    ui.invoke_apps_reload();
                });
            });
        }
    });
    ui.on_clean_leftovers({
        let (weak, pending, journal, last_ids) = (ui.as_weak(), pending.clone(), journal.clone(), last_ids.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let (name, found) = std::mem::take(&mut *pending.lock().unwrap());
            let (mut ids, mut errors) = (vec![], vec![]);
            for l in &found {
                match uninstall::clean_leftover(&mut journal.borrow_mut(), &name, l, &disk::stash_root()) {
                    Ok(id) => ids.push(id),
                    Err(e) => errors.push(e.to_string()),
                }
            }
            let mut msg = format!("{} restos movidos a la reserva. Se pueden devolver.", ids.len());
            if let Some(e) = errors.first() {
                msg = format!("{msg} No se pudieron limpiar {}: {e}", errors.len());
            }
            ui.set_can_undo_leftovers(!ids.is_empty());
            *last_ids.borrow_mut() = ids;
            ui.set_leftovers(ModelRc::new(VecModel::from(Vec::<DiskRow>::new())));
            ui.set_status(msg.into());
        }
    });
    ui.on_undo_leftovers({
        let (weak, journal, last_ids) = (ui.as_weak(), journal.clone(), last_ids.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let failed = last_ids.borrow().iter().rev().filter(|&&id| journal.borrow_mut().revert(id).is_err()).count();
            last_ids.borrow_mut().clear();
            ui.set_can_undo_leftovers(false);
            let msg = if failed == 0 { "Restos devueltos a su lugar.".to_string() } else { format!("{failed} restos no se pudieron devolver.") };
            ui.set_status(msg.into());
        }
    });
    ui.on_dismiss_leftovers({
        let (weak, pending) = (ui.as_weak(), pending.clone());
        move || {
            pending.lock().unwrap().1.clear();
            if let Some(ui) = weak.upgrade() {
                ui.set_leftovers(ModelRc::new(VecModel::from(Vec::<DiskRow>::new())));
                ui.set_status("Dejamos los restos como estaban.".into());
            }
        }
    });

    // Comportamiento sospechoso: línea base, persistencia, reverse shells, ofuscación y Defender manipulado.
    let behavior: Rc<RefCell<security::ScanResult>> = Rc::default();
    let shared: Arc<Mutex<Option<security::ScanResult>>> = Arc::default();
    let rebuild_findings = {
        let (weak, behavior, journal) = (ui.as_weak(), behavior.clone(), journal.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let (r, j) = (behavior.borrow(), journal.borrow());
            let mut total = 0.0;
            let rows: Vec<FindingRow> = r
                .findings
                .iter()
                .map(|f| {
                    total += finding_height(f);
                    let restorable = security::fix::disabled_id(&j, &f.key).is_some();
                    FindingRow {
                        title: f.title.clone().into(),
                        detail: f.detail.clone().into(),
                        decoded: f.decoded.clone().unwrap_or_default().into(),
                        severity: f.severity as i32,
                        can_disable: f.fix.is_some() && !restorable,
                        can_restore: restorable,
                    }
                })
                .collect();
            ui.set_findings(ModelRc::new(VecModel::from(rows)));
            ui.set_findings_h(total);
        }
    };
    ui.on_scan_behavior({
        let (weak, shared) = (ui.as_weak(), shared.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_behavior_busy(true);
            ui.set_sec_status("".into());
            ui.set_progress("Preparando la revisión".into());
            let (weak, shared) = (weak.clone(), shared.clone());
            thread::spawn(move || {
                // Cada etapa del análisis avisa qué está haciendo, con una frase corta.
                let report = {
                    let weak = weak.clone();
                    move |msg: &str| {
                        let msg = msg.to_string();
                        let _ = weak.upgrade_in_event_loop(move |ui| ui.set_progress(msg.into()));
                    }
                };
                // La lista de drivers peligrosos se renueva sola (como mucho una vez por semana).
                if hashdb::is_stale() {
                    report("Actualizando la lista de drivers peligrosos");
                }
                let refreshed = hashdb::ensure_fresh();
                let mut result = security::scan_with(&report);
                match refreshed {
                    Ok(true) => result.notes.insert(0, "Actualicé la lista de drivers peligrosos.".into()),
                    Err(_) if hashdb::Db::load().is_some() => result.notes.insert(0, "No pude conectarme para actualizar la lista de drivers peligrosos; usé la que ya tenía.".into()),
                    _ => {}
                }
                *shared.lock().unwrap() = Some(result);
                let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_behavior_ready());
            });
        }
    });
    ui.on_behavior_ready({
        let (weak, shared, behavior, rebuild) = (ui.as_weak(), shared.clone(), behavior.clone(), rebuild_findings.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            if let Some(r) = shared.lock().unwrap().take() {
                ui.set_sec_status(r.notes.join(" ").into());
                *behavior.borrow_mut() = r;
            }
            ui.set_behavior_busy(false);
            ui.set_behavior_run(true);
            rebuild();
            load_security(ui.as_weak()); // el estado de Defender y sus detecciones forman parte de los resultados
        }
    });
    ui.on_finding_do({
        let (weak, behavior, journal, rebuild) = (ui.as_weak(), behavior.clone(), journal.clone(), rebuild_findings.clone());
        move |i, action| {
            let Some(ui) = weak.upgrade() else { return };
            let Some(f) = behavior.borrow().findings.get(i as usize).cloned() else { return };
            let msg = match action {
                1 => match security::ignore(&behavior.borrow(), &f.key) {
                    Ok(()) => {
                        behavior.borrow_mut().findings.retain(|x| x.key != f.key);
                        "Ignorado: no se vuelve a mostrar.".to_string()
                    }
                    Err(e) => friendly(&e),
                },
                2 => match security::fix::disable(&mut journal.borrow_mut(), &f) {
                    Ok(_) => "Deshabilitado. Se guardó un respaldo y podés volver atrás con Restaurar.".to_string(),
                    Err(e) => friendly(&e),
                },
                _ => match security::fix::restore(&mut journal.borrow_mut(), &f.key) {
                    Ok(()) => "Restaurado: quedó como estaba.".to_string(),
                    Err(e) => friendly(&e),
                },
            };
            ui.set_sec_status(msg.into());
            rebuild();
        }
    });

    // Perfiles: encendido = el plan de lumen sigue siendo el activo. Apagar = deshacer ese cambio.
    let load_profiles = {
        let (weak, journal) = (ui.as_weak(), journal.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let active = power::active().unwrap_or_default();
            let j = journal.borrow();
            let row = |label: &str, desc: String, level: &str| ServiceRow {
                title: format!("Activar el {}", label.to_lowercase()).into(),
                desc: desc.into(),
                level: level.into(),
                tag: "".into(),
                enabled: profiles::active_entry(&j, label, &active).is_some(),
                locked: false,
            };
            let game = format!("Alto rendimiento y cierra {} apps de tu lista (game-list.json). Lo cerrado no se reabre solo.", profiles::game_list().len());
            let rows = vec![
                row(profiles::GAME, game, "depende"),
                row(profiles::BATTERY, "Plan de ahorro de energía para que la batería dure más.".into(), "seguro"),
            ];
            ui.set_profiles(ModelRc::new(VecModel::from(rows)));
        }
    };
    ui.on_toggle_profile({
        let (weak, journal, procs, reload) = (ui.as_weak(), journal.clone(), procs.clone(), load_profiles.clone());
        move |i, on| {
            let Some(ui) = weak.upgrade() else { return };
            let label = if i == 0 { profiles::GAME } else { profiles::BATTERY };
            let mut j = journal.borrow_mut();
            let mut msg = String::new();
            let res = if on {
                let action = if i == 0 { profiles::game_action() } else { profiles::battery_action() };
                action.and_then(|a| j.apply(label, a)).map(|_| {
                    if i == 0 {
                        msg = format!("{} procesos cerrados.", profiles::close_game_list(&mut procs.borrow_mut()));
                    }
                })
            } else {
                let active = power::active().unwrap_or_default();
                match profiles::active_entry(&j, label, &active) {
                    Some(id) => j.revert(id),
                    None => Err(std::io::Error::other("el plan de energía ya fue cambiado por otro lado")),
                }
            };
            if let Err(e) = res {
                msg = format!("No se pudo: {e}");
            }
            drop(j);
            ui.set_status(msg.into());
            reload();
        }
    });

    // Servicios: solo los de la base curada. Los críticos salen bloqueados.
    let svcs = Rc::new(RefCell::new(Vec::<services::Service>::new()));
    let load_services = {
        let (weak, svcs) = (ui.as_weak(), svcs.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            *svcs.borrow_mut() = services::list();
            let rows: Vec<ServiceRow> = svcs
                .borrow()
                .iter()
                .map(|s| ServiceRow {
                    title: s.title.clone().into(),
                    desc: s.desc.clone().into(),
                    level: match s.level {
                        services::Level::Safe => "seguro",
                        services::Level::Caution => "depende",
                        services::Level::Critical => "bloqueado",
                    }
                    .into(),
                    tag: "".into(),
                    enabled: !s.enabled(), // encendido = ya está desactivado
                    locked: s.locked(),
                })
                .collect();
            ui.set_services(ModelRc::new(VecModel::from(rows)));
        }
    };
    // Desactivar un servicio es un cambio grande: antes se crea un punto de restauración (en un hilo,
    // tarda unos segundos) y recién después se aplica. Reactivar es solo deshacer, sin punto.
    ui.on_toggle_service({
        let (weak, svcs, journal, reload) = (ui.as_weak(), svcs.clone(), journal.clone(), load_services.clone());
        move |i, on| {
            let Some(ui) = weak.upgrade() else { return };
            let name = svcs.borrow()[i as usize].name.clone();
            if on {
                ui.set_status("Creando punto de restauración, puede tardar unos segundos…".into());
                let weak = weak.clone();
                thread::spawn(move || {
                    let warn = match restore::create(&format!("desactivar servicio {name}")) {
                        Ok(()) => String::new(),
                        Err(e) => format!("Sin punto de restauración ({e}). El cambio igual se puede deshacer desde lumen."),
                    };
                    let _ = weak.upgrade_in_event_loop(move |ui| ui.invoke_disable_service(i, warn.into()));
                });
                return;
            }
            let mut j = journal.borrow_mut();
            // Volver a activar = deshacer el último cambio de lumen sobre este servicio.
            let last = j.entries.iter().rposition(|e| {
                e.applied && matches!(&e.action, Action::SetDword { path, .. } if path.ends_with(&format!(r"Services\{name}")))
            });
            let res = match last {
                Some(id) => j.revert(id),
                None => Err(std::io::Error::other("lumen no lo desactivó, no hay nada que revertir")),
            };
            drop(j);
            ui.set_status(service_msg(res, "").into());
            reload();
        }
    });
    ui.on_disable_service({
        let (weak, svcs, journal, reload) = (ui.as_weak(), svcs.clone(), journal.clone(), load_services.clone());
        move |i, warn| {
            let Some(ui) = weak.upgrade() else { return };
            let name = svcs.borrow()[i as usize].name.clone();
            let res = match services::disable_action(&name) {
                Some(a) => journal.borrow_mut().apply(&format!("Desactivar servicio: {name}"), a).map(|_| ()),
                None => Err(std::io::Error::other("este servicio está protegido")),
            };
            ui.set_status(service_msg(res, &warn).into());
            reload();
        }
    });

    // Disco: una sola acción. Los archivos pasan a una reserva que se vacía sola a los 7 días.
    let clean_ids = Rc::new(RefCell::new(Vec::<usize>::new()));
    let load_disk = {
        let weak = ui.as_weak();
        move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_disk_loading(true);
            let weak = weak.clone();
            thread::spawn(move || {
                disk::purge_older_than(&disk::stash_root(), 7);
                let parts: Vec<(&'static str, u64)> = disk::targets().iter().map(|t| (t.name, disk::dir_size(&t.path))).collect();
                let total: u64 = parts.iter().map(|p| p.1).sum();
                let old = disk::dir_size(&disk::stash_root());
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_disk_loading(false);
                    ui.set_disk_nothing(total < 1 << 20); // menos de 1 MB no vale la pena
                    ui.set_disk_total(size(total).into());
                    let line = parts.iter().filter(|p| p.1 >= 1 << 20).map(|(n, b)| format!("{n} {}", size(*b))).collect::<Vec<_>>().join(" · ");
                    ui.set_disk_parts(line.into());
                    let note = if old >= 1 << 20 { format!("Hay {} guardados de una limpieza anterior. Se borran solos a los 7 días.", size(old)) } else { String::new() };
                    ui.set_disk_old(note.into());
                });
            });
        }
    };
    ui.on_clean_all({
        let (weak, journal, ids) = (ui.as_weak(), journal.clone(), clean_ids.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_disk_phase(1);
            // Un instante para que se dibuje "Limpiando" antes de mover los archivos.
            let (weak, journal, ids) = (weak.clone(), journal.clone(), ids.clone());
            Timer::single_shot(Duration::from_millis(60), move || {
                let Some(ui) = weak.upgrade() else { return };
                let (mut bytes, mut skipped) = (0u64, 0usize);
                ids.borrow_mut().clear();
                for t in disk::targets() {
                    if let Ok(c) = disk::clean(&mut journal.borrow_mut(), t.name, &t.path, &disk::stash_root()) {
                        bytes += c.bytes;
                        skipped += c.skipped;
                        ids.borrow_mut().extend(c.id);
                    }
                }
                if bytes < 1 << 20 {
                    ui.set_disk_result("No había nada que se pudiera limpiar ahora: los archivos están en uso.".into());
                    ui.set_disk_phase(3);
                    return;
                }
                let mut msg = format!("Limpieza hecha: {}", size(bytes));
                if skipped > 0 {
                    msg += &format!(" ({skipped} elementos en uso quedaron)");
                }
                ui.set_disk_result(msg.into());
                ui.set_disk_phase(2);
            });
        }
    });
    ui.on_undo_clean({
        let (weak, journal, ids, reload) = (ui.as_weak(), journal.clone(), clean_ids.clone(), load_disk.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let failed = ids.borrow().iter().rev().filter(|&&id| journal.borrow_mut().revert(id).is_err()).count();
            ids.borrow_mut().clear();
            if failed > 0 {
                ui.set_disk_result(format!("No se pudieron devolver {failed} elementos.").into());
                ui.set_disk_phase(3);
            } else {
                ui.set_disk_phase(0);
                reload();
            }
        }
    });
    ui.on_purge({
        let weak = ui.as_weak();
        move || {
            let weak = weak.clone();
            thread::spawn(move || {
                let freed = disk::purge(&disk::stash_root());
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_disk_result(format!("Listo: liberaste {}.", size(freed)).into());
                    ui.set_disk_phase(3);
                });
            });
        }
    });
    ui.on_disk_reset({
        let (weak, ids, reload) = (ui.as_weak(), clean_ids.clone(), load_disk.clone());
        move || {
            ids.borrow_mut().clear();
            if let Some(ui) = weak.upgrade() {
                ui.set_disk_phase(0);
            }
            reload();
        }
    });
    ui.on_scan_map({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_space_map(ModelRc::new(VecModel::from(vec![DiskRow { name: "Calculando… puede tardar unos segundos".into(), size: "".into() }])));
            }
            let weak = weak.clone();
            thread::spawn(move || {
                let home = PathBuf::from(env::var_os("USERPROFILE").unwrap_or_default());
                let rows: Vec<(String, String)> = disk::space_map(&home)
                    .into_iter()
                    .take(15)
                    .map(|(p, s)| (p.file_name().unwrap_or_default().to_string_lossy().into_owned(), size(s)))
                    .collect();
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    let rows: Vec<DiskRow> = rows.into_iter().map(|(n, s)| DiskRow { name: n.into(), size: s.into() }).collect();
                    ui.set_space_map(ModelRc::new(VecModel::from(rows)));
                });
            });
        }
    });

    ui.on_opened({
        let (refresh, load_startup, load_disk, load_privacy, load_services, load_profiles, load_apps, weak) = (
            refresh.clone(),
            load_startup.clone(),
            load_disk.clone(),
            load_privacy.clone(),
            load_services.clone(),
            load_profiles.clone(),
            load_apps.clone(),
            ui.as_weak(),
        );
        move || match weak.upgrade().map(|u| u.get_page()) {
            Some(1) => refresh(),
            Some(2) => load_startup(),
            Some(3) => load_disk(),
            Some(4) => load_privacy(),
            Some(5) => load_services(),
            Some(6) => load_profiles(),
            Some(7) => load_apps(),
            _ => {}
        }
    });
    ui.on_kill({
        let (procs, refresh) = (procs.clone(), refresh.clone());
        move |pid| {
            procs.borrow_mut().kill(pid as u32);
            refresh();
        }
    });
    ui.run().unwrap();
}

#[cfg(test)]
mod tests {
    use super::{fmt_date, size, sync_rows};
    use slint::{Model, VecModel};

    #[test]
    fn sync_rows_grows_shrinks_and_updates_in_place() {
        let m = VecModel::from(vec![1, 2, 3]);
        sync_rows(&m, vec![9, 8, 7, 6]);
        assert_eq!(m.iter().collect::<Vec<_>>(), [9, 8, 7, 6]);
        sync_rows(&m, vec![5]);
        assert_eq!(m.iter().collect::<Vec<_>>(), [5]);
        sync_rows(&m, vec![]);
        assert_eq!(m.row_count(), 0);
    }

    #[test]
    fn fmt_date_formats_defender_timestamps() {
        assert_eq!(fmt_date("2026-10-15T11:46:03.0000000-03:00"), "15/10/2026 11:46");
        assert_eq!(fmt_date(""), "nunca");
    }

    #[test]
    fn size_uses_gb_from_one_gb_and_mb_below() {
        assert_eq!(size(1369 * 1_048_576), "1,3 GB");
        assert_eq!(size(3355 * 1_048_576), "3,3 GB");
        assert_eq!(size(900 * 1_048_576), "900 MB");
        assert_eq!(size(1_073_741_824), "1,0 GB");
        assert_eq!(size(0), "0 MB");
        assert_eq!(size(12 * 1024), "<1 MB");
    }
}
