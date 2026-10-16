//! Ayudantes para consultar a Windows con PowerShell (sin ventana) y leer su salida JSON.
use serde_json::Value;
use std::{io, os::windows::process::CommandExt, process::Command};

/// Ejecuta un script y devuelve su salida. La consola se fuerza a UTF-8 para no romper rutas con acentos.
pub fn run(script: &str) -> io::Result<String> {
    let full = format!("[Console]::OutputEncoding = [Text.Encoding]::UTF8; {script}");
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &full])
        // Si lumen se abre desde PowerShell 7, hereda su PSModulePath y Windows PowerShell 5.1 no puede
        // cargar sus propios módulos (Security, Defender...). Sin la variable usa la ruta correcta.
        .env_remove("PSModulePath")
        .creation_flags(0x0800_0000)
        .output()?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(io::Error::other(err.lines().find(|l| !l.trim().is_empty()).unwrap_or("error de PowerShell").trim().to_string()))
    }
}

/// PowerShell devuelve un objeto si hay uno solo y una lista si hay varios: se normaliza a lista.
pub fn rows(json: &str) -> Vec<Value> {
    match serde_json::from_str::<Value>(json) {
        Ok(Value::Array(a)) => a,
        Ok(v @ Value::Object(_)) => vec![v],
        _ => vec![],
    }
}

pub fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

pub fn flag(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// Un campo que PowerShell entrega como texto, lista de textos o nada.
pub fn strings(v: &Value, k: &str) -> Vec<String> {
    match v.get(k) {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => vec![],
    }
}

/// Texto entre comillas simples para incrustar en un script (duplica las comillas internas).
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_object_and_array() {
        assert_eq!(rows(r#"{"a":1}"#).len(), 1);
        assert_eq!(rows(r#"[{"a":1},{"a":2}]"#).len(), 2);
        assert!(rows("").is_empty());
    }

    #[test]
    fn reads_string_or_list_fields() {
        let v: Value = serde_json::from_str(r#"{"x":"uno","y":["a","b"],"z":null}"#).unwrap();
        assert_eq!(strings(&v, "x"), ["uno"]);
        assert_eq!(strings(&v, "y"), ["a", "b"]);
        assert!(strings(&v, "z").is_empty());
    }

    #[test]
    fn quotes_apostrophes() {
        assert_eq!(quote("it's"), "'it''s'");
    }
}
