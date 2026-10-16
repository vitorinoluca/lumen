#![windows_subsystem = "windows"]
use lumen_core::monitor::{Monitor, Snapshot};
use std::{thread, time::Duration};

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
    let weak = ui.as_weak();
    // Lectura en hilo aparte: la UI nunca se bloquea.
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
    ui.run().unwrap();
}
