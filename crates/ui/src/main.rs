#![windows_subsystem = "windows"]
use lumen_core::{
    monitor::{Monitor, Snapshot},
    processes::{Processes, SortBy},
};
use slint::{ModelRc, Timer, TimerMode, VecModel};
use std::{cell::Cell, rc::Rc, thread, time::Duration};

slint::include_modules!();

fn gb(b: u64) -> String {
    format!("{:.1} GB", b as f64 / 1e9)
}

fn show(ui: &Main, s: &Snapshot) {
    let frac = |a: u64, b: u64| if b == 0 { 0.0 } else { a as f32 / b as f32 };
    ui.set_cpu(s.cpu / 100.0);
    ui.set_ram(frac(s.ram_used, s.ram_total));
    ui.set_ram_text(format!("{} / {}", gb(s.ram_used), gb(s.ram_total)).into());
    ui.set_disk(frac(s.disk_used, s.disk_total));
    ui.set_disk_text(format!("{} / {}", gb(s.disk_used), gb(s.disk_total)).into());
    ui.set_net_text(format!("↓ {} KB/s  ↑ {} KB/s", s.net_rx / 1024, s.net_tx / 1024).into());
}

fn main() {
    let ui = Main::new().unwrap();

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
    ui.on_opened(refresh.clone());
    ui.on_kill({
        let (procs, refresh) = (procs.clone(), refresh.clone());
        move |pid| {
            procs.borrow_mut().kill(pid as u32);
            refresh();
        }
    });
    ui.run().unwrap();
}
