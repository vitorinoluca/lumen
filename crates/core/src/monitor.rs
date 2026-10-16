//! Monitor de CPU, RAM, disco y red.
use sysinfo::{Disks, Networks, System};
use windows_sys::Win32::{Foundation::FILETIME, System::Threading::GetSystemTimes};

/// (inactivo, total) acumulados del sistema. Se usa esto y no `sysinfo` porque
/// su uso de CPU global se queda fijo en 100% en algunas versiones de Windows.
fn cpu_times() -> (u64, u64) {
    let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut idle, mut kernel, mut user) = (z, z, z);
    // SAFETY: punteros a variables locales válidas.
    unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    let n = |f: FILETIME| (f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64;
    (n(idle), n(kernel) + n(user)) // kernel ya incluye el tiempo inactivo
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub cpu: f32,
    pub ram_used: u64,
    pub ram_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
    /// Bytes recibidos/enviados desde la muestra anterior.
    pub net_rx: u64,
    pub net_tx: u64,
}

pub struct Monitor {
    sys: System,
    last_cpu: (u64, u64),
    disks: Disks,
    nets: Networks,
}

impl Monitor {
    pub fn new() -> Self {
        Self { sys: System::new(), last_cpu: cpu_times(), disks: Disks::new_with_refreshed_list(), nets: Networks::new_with_refreshed_list() }
    }

    /// Toma una muestra. La CPU necesita dos muestras separadas para dar un valor real.
    pub fn sample(&mut self) -> Snapshot {
        let now = cpu_times();
        let (idle, total) = (now.0 - self.last_cpu.0, now.1 - self.last_cpu.1);
        self.last_cpu = now;
        let cpu = if total == 0 { 0.0 } else { 100.0 * (1.0 - idle as f32 / total as f32) };
        self.sys.refresh_memory();
        self.disks.refresh(true);
        self.nets.refresh(true);
        let (disk_total, free) = self.disks.list().iter().fold((0, 0), |(t, f), d| (t + d.total_space(), f + d.available_space()));
        Snapshot {
            cpu,
            ram_used: self.sys.used_memory(),
            ram_total: self.sys.total_memory(),
            disk_used: disk_total - free,
            disk_total,
            net_rx: self.nets.values().map(|n| n.received()).sum(),
            net_tx: self.nets.values().map(|n| n.transmitted()).sum(),
        }
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_has_sane_values() {
        let s = Monitor::new().sample();
        assert!(s.ram_total > 0 && s.ram_used <= s.ram_total);
        assert!(s.disk_used <= s.disk_total);
        assert!((0.0..=100.0).contains(&s.cpu));
    }
}
