//! Datos fijos del equipo para el panel: sistema, procesador, placa, GPU y discos.
use sysinfo::{Disks, System};
use winreg::{enums::*, RegKey};

#[derive(Debug, Clone, Default)]
pub struct DiskInfo {
    pub mount: String,
    pub kind: String,
    pub fs: String,
    pub total: u64,
    pub used: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Hardware {
    pub os: String,
    pub os_build: String,
    pub cpu: String,
    pub cores: usize,
    pub threads: usize,
    pub mhz: u64,
    pub ram_total: u64,
    pub gpu: String,
    pub vram: u64,
    pub board: String,
    pub uptime_secs: u64,
    pub disks: Vec<DiskInfo>,
}

fn reg_text(path: &str, name: &str) -> String {
    RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(path).and_then(|k| k.get_value::<String, _>(name)).unwrap_or_default().trim().to_string()
}

/// GPU con más memoria de video registrada (evita elegir la integrada si hay una dedicada).
fn gpu() -> (String, u64) {
    let class = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let Ok(base) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(class) else { return Default::default() };
    base.enum_keys()
        .flatten()
        .filter(|k| k.len() == 4 && k.chars().all(|c| c.is_ascii_digit()))
        .filter_map(|k| {
            let key = base.open_subkey(&k).ok()?;
            let name: String = key.get_value("DriverDesc").ok()?;
            let mem = key.get_value::<u64, _>("HardwareInformation.qwMemorySize").unwrap_or(0);
            Some((name, mem))
        })
        .max_by_key(|(_, mem)| *mem)
        .unwrap_or_default()
}

pub fn read() -> Hardware {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();
    let cpus = sys.cpus();
    let bios = r"HARDWARE\DESCRIPTION\System\BIOS";
    let (gpu, vram) = gpu();
    let disks = Disks::new_with_refreshed_list()
        .list()
        .iter()
        .map(|d| DiskInfo {
            mount: d.mount_point().display().to_string(),
            kind: d.kind().to_string(),
            fs: d.file_system().to_string_lossy().into_owned(),
            total: d.total_space(),
            used: d.total_space() - d.available_space(),
        })
        .collect();
    Hardware {
        os: System::long_os_version().unwrap_or_default(),
        os_build: System::os_version().unwrap_or_default(),
        cpu: cpus.first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        cores: System::physical_core_count().unwrap_or(0),
        threads: cpus.len(),
        mhz: cpus.first().map_or(0, |c| c.frequency()),
        ram_total: sys.total_memory(),
        gpu,
        vram,
        board: format!("{} {}", reg_text(bios, "BaseBoardManufacturer"), reg_text(bios, "BaseBoardProduct")).trim().to_string(),
        uptime_secs: System::uptime(),
        disks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_basic_machine_info() {
        let h = read();
        assert!(h.os.contains("Windows"), "os = {}", h.os);
        assert!(!h.cpu.is_empty() && h.threads > 0 && h.ram_total > 0);
        assert!(h.disks.iter().any(|d| d.total > 0));
    }
}
