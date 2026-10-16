//! Revisión de seguridad local: detecta comportamiento sospechoso que Windows Defender pasa por alto.
//! Solo lectura: no cambia nada sin que el usuario lo confirme. Sin IA ni servicios externos.
//!
//! Funciona con una línea base: el primer escaneo guarda lo que hay y los siguientes avisan
//! solo de lo nuevo o modificado. Lo que es sospechoso por sí mismo (reverse shells, procesos
//! falsos, Defender apagado) se informa siempre, hasta que el usuario lo ignore.
use crate::actions::Hive;
use crate::startup::Source;

pub mod analyze;
pub mod collect;
pub mod signature;

/// Orden de aparición: alta primero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    High,
    Medium,
    Low,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::High => "alta",
            Severity::Medium => "media",
            Severity::Low => "baja",
        }
    }
}

/// Cómo desactivar el origen de un hallazgo. Siempre con confirmación y reversible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// Entrada de inicio (clave Run del usuario o del equipo, o carpeta Inicio del usuario).
    Startup { source: Source, name: String },
    /// Tarea programada.
    Task { path: String, name: String },
    /// Servicio: se pone en "deshabilitado".
    Service { name: String },
    /// Valor de registro de texto: se borra (`restore: None`) o se vuelve al valor esperado.
    RegText { hive: Hive, path: String, name: String, restore: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Identificador estable: sirve para ignorar el hallazgo y reconocerlo en el próximo escaneo.
    pub key: String,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    /// Comando real, si estaba codificado en base64.
    pub decoded: Option<String>,
    pub fix: Option<Fix>,
}
