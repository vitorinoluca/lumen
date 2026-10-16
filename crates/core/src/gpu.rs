//! Uso de la GPU con el contador de rendimiento de Windows "GPU Engine" (sirve para cualquier marca).
use std::{collections::HashMap, ptr};
use windows_sys::Win32::System::Performance::*;

/// Cada cuántas muestras se rearma la consulta, para tomar los procesos que aparecieron después.
const REFRESH: u32 = 30;

pub struct Gpu {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
    age: u32,
}

impl Gpu {
    pub fn new() -> Self {
        let mut g = Self { query: ptr::null_mut(), counter: ptr::null_mut(), age: 0 };
        g.open();
        g
    }

    fn open(&mut self) {
        self.close();
        let path: Vec<u16> = r"\GPU Engine(*)\Utilization Percentage".encode_utf16().chain([0]).collect();
        // SAFETY: punteros a variables locales válidas; si algo falla queda todo en nulo y `sample` da `None`.
        unsafe {
            if PdhOpenQueryW(ptr::null(), 0, &mut self.query) != 0
                || PdhAddEnglishCounterW(self.query, path.as_ptr(), 0, &mut self.counter) != 0
            {
                self.close();
                return;
            }
            PdhCollectQueryData(self.query);
        }
    }

    fn close(&mut self) {
        if !self.query.is_null() {
            // SAFETY: la consulta se abrió en `open` y no se cierra en otro lado.
            unsafe { PdhCloseQuery(self.query) };
        }
        self.query = ptr::null_mut();
        self.counter = ptr::null_mut();
    }

    /// Porcentaje de uso (0-100), o `None` si la PC no tiene el contador o aún no hay dos muestras.
    pub fn sample(&mut self) -> Option<f32> {
        self.age += 1;
        let out = self.read();
        if self.age >= REFRESH {
            self.age = 0;
            self.open();
        }
        out
    }

    fn read(&self) -> Option<f32> {
        if self.query.is_null() {
            return None;
        }
        let (mut bytes, mut n) = (0u32, 0u32);
        // SAFETY: primero se pide el tamaño y después se llama con un búfer de ese tamaño (alineado a 8).
        let items = unsafe {
            PdhCollectQueryData(self.query);
            if PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut bytes, &mut n, ptr::null_mut()) != PDH_MORE_DATA {
                return None;
            }
            let mut buf = vec![0u64; bytes as usize / 8 + 1];
            let p = buf.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            if PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut bytes, &mut n, p) != 0 {
                return None;
            }
            (0..n as usize)
                .filter_map(|i| {
                    let it = *p.add(i);
                    let ok = it.FmtValue.CStatus == PDH_CSTATUS_VALID_DATA || it.FmtValue.CStatus == PDH_CSTATUS_NEW_DATA;
                    let (mut len, name) = (0, it.szName);
                    while *name.add(len) != 0 {
                        len += 1;
                    }
                    ok.then(|| (String::from_utf16_lossy(std::slice::from_raw_parts(name, len)), it.FmtValue.Anonymous.doubleValue))
                })
                .collect::<Vec<_>>()
        };
        usage(&items)
    }
}

/// Suma por tipo de motor (3D, Copy, Video...) y toma el más ocupado, como el Administrador de tareas.
fn usage(items: &[(String, f64)]) -> Option<f32> {
    let mut by_type: HashMap<&str, f64> = HashMap::new();
    for (name, v) in items {
        *by_type.entry(name.rsplit("engtype_").next().unwrap_or("")).or_default() += v;
    }
    by_type.values().copied().reduce(f64::max).map(|v| v.clamp(0.0, 100.0) as f32)
}

impl Drop for Gpu {
    fn drop(&mut self) {
        self.close();
    }
}

impl Default for Gpu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_busiest_engine_type() {
        let items = vec![
            ("pid_1_luid_0x0_phys_0_eng_0_engtype_3D".to_string(), 30.0),
            ("pid_2_luid_0x0_phys_0_eng_0_engtype_3D".to_string(), 25.0),
            ("pid_2_luid_0x0_phys_0_eng_3_engtype_Copy".to_string(), 10.0),
        ];
        assert_eq!(usage(&items), Some(55.0));
        assert_eq!(usage(&[]), None);
    }

    #[test]
    fn sample_does_not_crash() {
        let mut g = Gpu::new();
        let _ = g.sample();
        if let Some(v) = g.sample() {
            assert!((0.0..=100.0).contains(&v));
        }
    }
}
