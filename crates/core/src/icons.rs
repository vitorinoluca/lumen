//! Íconos de programas: se sacan del exe/ico que indica Windows y se devuelven como píxeles RGBA.
use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr};
use windows_sys::Win32::{
    Graphics::Gdi::{CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS},
    UI::{
        Shell::ExtractIconExW,
        WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO},
    },
};

pub struct Icon {
    pub width: u32,
    pub height: u32,
    /// RGBA, 4 bytes por píxel.
    pub rgba: Vec<u8>,
}

/// Reemplaza `%VAR%` por el valor de la variable de entorno.
fn expand(s: &str) -> String {
    let mut out = String::new();
    let mut parts = s.split('%');
    out.push_str(parts.next().unwrap_or(""));
    while let Some(name) = parts.next() {
        match (std::env::var(name), parts.next()) {
            (Ok(v), Some(rest)) => {
                out.push_str(&v);
                out.push_str(rest);
            }
            (_, rest) => {
                out.push('%');
                out.push_str(name);
                if let Some(r) = rest {
                    out.push('%');
                    out.push_str(r);
                }
            }
        }
    }
    out
}

/// `"C:\ruta\app.exe",2` / `C:\ruta\app.ico` / `C:\ruta\app.exe,-101` -> (ruta, índice)
pub fn parse_spec(spec: &str) -> (String, i32) {
    let s = expand(spec.trim());
    let (path, rest) = match s.strip_prefix('"') {
        Some(r) => r.split_once('"').unwrap_or((r, "")),
        None => match s.rsplit_once(',') {
            Some((p, i)) if i.trim().parse::<i32>().is_ok() => (p, &s[p.len()..]),
            _ => (s.as_str(), ""),
        },
    };
    let index = rest.trim_start_matches(',').trim().parse().unwrap_or(0);
    (path.trim().to_string(), index)
}

/// Primer ejecutable de una línea de comando, por si el programa no declara ícono.
pub fn exe_of_command(cmd: &str) -> Option<String> {
    let c = expand(cmd.trim());
    let path = match c.strip_prefix('"') {
        Some(r) => r.split_once('"')?.0.to_string(),
        None => c[..c.to_lowercase().find(".exe")? + 4].to_string(),
    };
    path.to_lowercase().ends_with(".exe").then_some(path)
}

/// Carga el ícono. None si el archivo no existe o no tiene ícono.
pub fn load(spec: &str) -> Option<Icon> {
    let (path, index) = parse_spec(spec);
    if path.is_empty() || !std::path::Path::new(&path).exists() {
        return None;
    }
    let wide: Vec<u16> = std::ffi::OsStr::new(&path).encode_wide().chain(Some(0)).collect();
    let mut big = ptr::null_mut();
    // SAFETY: `wide` termina en 0 y `big` es un puntero a una variable local válida.
    let n = unsafe { ExtractIconExW(wide.as_ptr(), index, &mut big, ptr::null_mut(), 1) };
    if n == 0 || big.is_null() {
        return None;
    }
    let icon = unsafe { to_rgba(big) };
    // SAFETY: `big` es un HICON que devolvió ExtractIconExW y no se usa más.
    unsafe { DestroyIcon(big) };
    icon
}

/// SAFETY: `hicon` debe ser un HICON válido.
unsafe fn to_rgba(hicon: *mut c_void) -> Option<Icon> {
    let mut info: ICONINFO = std::mem::zeroed();
    if GetIconInfo(hicon, &mut info) == 0 {
        return None;
    }
    let mut bm: BITMAP = std::mem::zeroed();
    GetObjectW(info.hbmColor, std::mem::size_of::<BITMAP>() as i32, &mut bm as *mut _ as *mut c_void);
    let (w, h) = (bm.bmWidth, bm.bmHeight);
    let result = if w > 0 && h > 0 {
        let dc = CreateCompatibleDC(ptr::null_mut());
        let read = |bitmap| {
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // de arriba hacia abajo
                biPlanes: 1,
                biBitCount: 32,
                ..std::mem::zeroed()
            };
            let mut buf = vec![0u8; (w * h * 4) as usize];
            let ok = GetDIBits(dc, bitmap, 0, h as u32, buf.as_mut_ptr() as *mut c_void, &mut bmi, DIB_RGB_COLORS);
            (ok != 0).then_some(buf)
        };
        let color = read(info.hbmColor);
        let mask = read(info.hbmMask);
        DeleteDC(dc);
        color.map(|mut px| {
            // Los íconos viejos no traen canal alfa: ahí manda la máscara (negro = opaco).
            let has_alpha = px.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
            for (i, p) in px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                p.swap(0, 2); // BGRA -> RGBA
                if !has_alpha {
                    p[3] = if mask.as_ref().is_some_and(|m| m[i * 4] != 0) { 0 } else { 255 };
                }
            }
            Icon { width: w as u32, height: h as u32, rgba: px }
        })
    } else {
        None
    };
    DeleteObject(info.hbmColor);
    DeleteObject(info.hbmMask);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_icon_specs() {
        assert_eq!(parse_spec(r#""C:\a b\app.exe",2"#), (r"C:\a b\app.exe".to_string(), 2));
        assert_eq!(parse_spec(r"C:\app\icon.ico"), (r"C:\app\icon.ico".to_string(), 0));
        assert_eq!(parse_spec(r"C:\app\x.exe,-101"), (r"C:\app\x.exe".to_string(), -101));
        assert_eq!(parse_spec(r#""C:\x\y.exe""#), (r"C:\x\y.exe".to_string(), 0));
    }

    #[test]
    fn finds_exe_in_command_lines() {
        assert_eq!(exe_of_command(r#""C:\Program Files\X\unins000.exe" /SILENT"#).as_deref(), Some(r"C:\Program Files\X\unins000.exe"));
        assert_eq!(exe_of_command(r"C:\X\un.exe /S").as_deref(), Some(r"C:\X\un.exe"));
        assert_eq!(exe_of_command("MsiExec.exe /X{ABC}").as_deref(), Some("MsiExec.exe"));
        assert_eq!(exe_of_command("sin ejecutable"), None);
    }

    #[test]
    fn extracts_a_real_icon() {
        let i = load(r"%SystemRoot%\System32\notepad.exe,0").expect("notepad tiene ícono");
        assert!(i.width > 0 && i.rgba.len() == (i.width * i.height * 4) as usize);
        assert!(i.rgba.as_chunks::<4>().0.iter().any(|p| p[3] > 0), "debe haber píxeles visibles");
    }
}
