//! Idioma de los textos que arma Rust. Los de las pantallas los traduce Slint con el mismo `lumen.po`.
//! Los textos del programa están en español: en inglés se busca la frase (o su molde con `{}`) en el .po.
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};

const EN_PO: &str = include_str!("../lang/en/LC_MESSAGES/lumen.po");

static ENGLISH: AtomicBool = AtomicBool::new(false);
/// Frases exactas y moldes con `{}`.
type Dict = (HashMap<String, String>, Vec<(String, String)>);

static DICT: OnceLock<Dict> = OnceLock::new();

pub fn set_english(on: bool) {
    ENGLISH.store(on, Ordering::Relaxed);
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').replace("\\\"", "\"").replace("\\n", "\n").replace("\\\\", "\\")
}

/// Lee el .po: (frases exactas, moldes con `{}`).
fn parse(po: &str) -> Dict {
    let (mut exact, mut templates) = (HashMap::new(), Vec::new());
    let mut id: Option<String> = None;
    for line in po.lines() {
        if let Some(v) = line.strip_prefix("msgid ") {
            id = Some(unquote(v));
        } else if let Some(v) = line.strip_prefix("msgstr ") {
            if let Some(k) = id.take().filter(|k| !k.is_empty()) {
                let v = unquote(v);
                if k.contains("{}") {
                    templates.push((k, v));
                } else {
                    exact.insert(k, v);
                }
            }
        }
    }
    (exact, templates)
}

/// Si `text` encaja con el molde (`{}` = cualquier cosa), devuelve lo que ocupó cada `{}`.
fn captures<'a>(template: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let parts: Vec<&str> = template.split("{}").collect();
    let mut rest = text.strip_prefix(parts[0])?;
    let mut out = Vec::new();
    for (i, lit) in parts.iter().enumerate().skip(1) {
        let last = i == parts.len() - 1;
        let at = if last { rest.strip_suffix(lit).map(|r| r.len()) } else { rest.find(lit).filter(|_| !lit.is_empty()) }?;
        out.push(&rest[..at]);
        rest = &rest[at + lit.len()..];
    }
    rest.is_empty().then_some(out)
}

fn lookup(text: &str) -> String {
    let (exact, templates) = DICT.get_or_init(|| parse(EN_PO));
    if let Some(v) = exact.get(text) {
        return v.clone();
    }
    for (k, v) in templates {
        if let Some(args) = captures(k, text) {
            let mut args = args.into_iter();
            let mut out = String::new();
            for (i, piece) in v.split("{}").enumerate() {
                if i > 0 {
                    out += &lookup(args.next().unwrap_or(""));
                }
                out += piece;
            }
            return out;
        }
    }
    // Textos de varias líneas: cada línea por su cuenta (los hallazgos mezclan frases fijas y datos).
    if text.contains('\n') {
        return text.split('\n').map(lookup).collect::<Vec<_>>().join("\n");
    }
    text.to_string()
}

/// Traduce un texto en español al idioma elegido; si no lo conoce, lo deja como está.
pub fn t(text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    if ENGLISH.load(Ordering::Relaxed) { lookup(text) } else { text.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_exact_phrases_and_templates() {
        assert_eq!(lookup("Cancelar"), "Cancel");
        assert_eq!(lookup("3 procesos"), "3 processes");
        assert_eq!(lookup("Aplicaste 2 de 9 mejoras recomendadas"), "You applied 2 of 9 recommended improvements");
        assert_eq!(lookup("Frase desconocida"), "Frase desconocida");
    }
}
