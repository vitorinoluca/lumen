//! Lista de procesos y finalización.
use std::path::PathBuf;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Debug, Clone)]
pub struct Proc {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem: u64,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortBy {
    Cpu,
    Mem,
    Name,
}

#[derive(Default)]
pub struct Processes {
    sys: System,
}

impl Processes {
    /// Lista ordenada. El % de CPU necesita dos llamadas separadas para ser real.
    pub fn list(&mut self, sort: SortBy) -> Vec<Proc> {
        let what = ProcessRefreshKind::nothing().with_cpu().with_memory().with_exe(sysinfo::UpdateKind::OnlyIfNotSet);
        self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, what);
        let mut v: Vec<Proc> = self
            .sys
            .processes()
            .values()
            .map(|p| Proc {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu: p.cpu_usage(),
                mem: p.memory(),
                path: p.exe().map(PathBuf::from),
            })
            .collect();
        match sort {
            SortBy::Cpu => v.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
            SortBy::Mem => v.sort_by_key(|p| std::cmp::Reverse(p.mem)),
            SortBy::Name => v.sort_by_key(|p| p.name.to_lowercase()),
        }
        v
    }

    /// Finaliza todos los procesos con esos nombres (sin distinguir mayúsculas). Devuelve cuántos.
    pub fn kill_named(&mut self, names: &[String]) -> usize {
        self.list(SortBy::Name); // refresca
        self.sys
            .processes()
            .values()
            .filter(|p| names.iter().any(|n| p.name().to_string_lossy().eq_ignore_ascii_case(n)))
            .filter(|p| p.kill())
            .count()
    }

    /// Finaliza un proceso. No es reversible: la UI debe pedir confirmación.
    pub fn kill(&mut self, pid: u32) -> bool {
        self.sys.process(Pid::from_u32(pid)).is_some_and(|p| p.kill())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_self_and_sorts() {
        let mut p = Processes::default();
        let by_mem = p.list(SortBy::Mem);
        assert!(by_mem.iter().any(|x| x.pid == std::process::id()));
        assert!(by_mem.windows(2).all(|w| w[0].mem >= w[1].mem));
        let by_name = p.list(SortBy::Name);
        assert!(by_name.windows(2).all(|w| w[0].name.to_lowercase() <= w[1].name.to_lowercase()));
    }
}
