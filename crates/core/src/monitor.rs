//! Monitor de CPU, RAM, disco y red.
use sysinfo::{Disks, Networks, System};

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
    disks: Disks,
    nets: Networks,
}

impl Monitor {
    pub fn new() -> Self {
        Self { sys: System::new(), disks: Disks::new_with_refreshed_list(), nets: Networks::new_with_refreshed_list() }
    }

    /// Toma una muestra. La CPU necesita dos muestras separadas para dar un valor real.
    pub fn sample(&mut self) -> Snapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.disks.refresh(true);
        self.nets.refresh(true);
        let (disk_total, free) = self.disks.list().iter().fold((0, 0), |(t, f), d| (t + d.total_space(), f + d.available_space()));
        Snapshot {
            cpu: self.sys.global_cpu_usage(),
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
    }
}
