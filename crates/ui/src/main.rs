#![windows_subsystem = "windows"]
use lumen_core::{
    actions::{Action, Hive, Journal},
    monitor::{Monitor, Snapshot},
    processes::{Processes, SortBy},
    startup::{self, Source},
    disk,
    hardware, power, privacy, profiles, restore, services,
    uninstall::{self, Leftover},
};
use slint::{ModelRc, Timer, TimerMode, VecModel};
use std::{cell::{Cell, RefCell}, env, path::PathBuf, sync::{Arc, Mutex}, rc::Rc, thread, time::Duration};

slint::include_modules!();

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

fn mb(b: u64) -> String {
    format!("{} MB", b / 1_048_576)
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
    let sort = Rc::new(Cell::new(SortBy::Cpu));
    let refresh = {
        let (weak, procs, sort) = (ui.as_weak(), procs.clone(), sort.clone());
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
            ui.set_procs(ModelRc::new(VecModel::from(rows)));
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
    let apps = Rc::new(RefCell::new(Vec::<uninstall::App>::new()));
    let pending: Arc<Mutex<(String, Vec<Leftover>)>> = Arc::default();
    let last_ids = Rc::new(RefCell::new(Vec::<usize>::new()));
    let load_apps = {
        let (weak, apps) = (ui.as_weak(), apps.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            *apps.borrow_mut() = uninstall::list();
            let rows: Vec<AppRow> = apps
                .borrow()
                .iter()
                .map(|a| AppRow {
                    name: a.name.clone().into(),
                    detail: [a.publisher.as_str(), a.version.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ").into(),
                    size: if a.size_kb > 0 { mb(a.size_kb * 1024) } else { String::new() }.into(),
                })
                .collect();
            ui.set_apps(ModelRc::new(VecModel::from(rows)));
        }
    };
    ui.on_apps_reload(load_apps.clone());
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

    // Perfiles: encendido = el plan de lumen sigue siendo el activo. Apagar = deshacer ese cambio.
    let load_profiles = {
        let (weak, journal) = (ui.as_weak(), journal.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let active = power::active().unwrap_or_default();
            let j = journal.borrow();
            let row = |label: &str, desc: String, level: &str| ServiceRow {
                title: label.into(),
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
                    enabled: s.enabled(),
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
            if !on {
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

    // Disco: los tamaños se calculan en un hilo; limpiar y deshacer son renames rápidos.
    let load_disk = {
        let weak = ui.as_weak();
        move || {
            let weak = weak.clone();
            thread::spawn(move || {
                let rows: Vec<(String, String)> =
                    disk::targets().iter().map(|t| (t.name.into(), mb(disk::dir_size(&t.path)))).collect();
                let stash = mb(disk::dir_size(&disk::stash_root()));
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    let rows: Vec<DiskRow> = rows.into_iter().map(|(n, s)| DiskRow { name: n.into(), size: s.into() }).collect();
                    ui.set_targets(ModelRc::new(VecModel::from(rows)));
                    ui.set_stash_text(stash.into());
                });
            });
        }
    };
    ui.on_clean({
        let (weak, journal, reload) = (ui.as_weak(), journal.clone(), load_disk.clone());
        move |i| {
            let Some(ui) = weak.upgrade() else { return };
            let t = &disk::targets()[i as usize];
            let msg = match disk::clean(&mut journal.borrow_mut(), t.name, &t.path, &disk::stash_root()) {
                Ok((n, skip)) => format!("{} elementos movidos a la reserva, {skip} en uso omitidos", n),
                Err(e) => format!("No se pudo: {e}"),
            };
            ui.set_disk_status(msg.into());
            reload();
        }
    });
    ui.on_undo_clean({
        let (weak, journal, reload) = (ui.as_weak(), journal.clone(), load_disk.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let mut j = journal.borrow_mut();
            let last = j.entries.iter().rposition(|e| e.applied && matches!(e.action, Action::Stash { .. }));
            let msg = match last.map(|id| j.revert(id)) {
                Some(Ok(())) => "Limpieza deshecha".to_string(),
                Some(Err(e)) => format!("No se pudo deshacer: {e}"),
                None => "No hay limpiezas para deshacer".to_string(),
            };
            ui.set_disk_status(msg.into());
            reload();
        }
    });
    ui.on_purge({
        let (weak, reload) = (ui.as_weak(), load_disk.clone());
        move || {
            let weak = weak.clone();
            let reload = reload.clone();
            thread::spawn(move || {
                let freed = disk::purge(&disk::stash_root());
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_disk_status(format!("Liberados {}", mb(freed)).into());
                    reload();
                });
            });
        }
    });
    ui.on_scan_map({
        let weak = ui.as_weak();
        move || {
            let weak = weak.clone();
            thread::spawn(move || {
                let home = PathBuf::from(env::var_os("USERPROFILE").unwrap_or_default());
                let rows: Vec<(String, String)> = disk::space_map(&home)
                    .into_iter()
                    .take(15)
                    .map(|(p, s)| (p.file_name().unwrap_or_default().to_string_lossy().into_owned(), mb(s)))
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
