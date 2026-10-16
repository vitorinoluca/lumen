//! Configuración de la app: tema, frecuencia del panel e idioma.
//! Se guarda en `%APPDATA%\lumen\config.json`. Si el archivo falta o está dañado se usan los valores por defecto.
use serde::{Deserialize, Serialize};
use std::{env, fs, io, path::{Path, PathBuf}};

/// Opciones válidas de refresco del panel, en segundos.
pub const REFRESH_CHOICES: [u64; 3] = [1, 2, 5];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    /// "auto" (sigue a Windows), "light" o "dark".
    pub theme: String,
    /// Cada cuántos segundos se actualiza el panel.
    pub refresh_secs: u64,
    /// "es" o "en".
    pub language: String,
}

impl Default for Config {
    fn default() -> Self {
        Config { theme: "auto".into(), refresh_secs: 1, language: "es".into() }
    }
}

impl Config {
    /// Deja valores válidos: lo desconocido vuelve al valor por defecto.
    pub fn sanitized(mut self) -> Config {
        let d = Config::default();
        if !["auto", "light", "dark"].contains(&self.theme.as_str()) {
            self.theme = d.theme;
        }
        if !REFRESH_CHOICES.contains(&self.refresh_secs) {
            self.refresh_secs = d.refresh_secs;
        }
        if !["es", "en"].contains(&self.language.as_str()) {
            self.language = d.language;
        }
        self
    }

    pub fn path() -> PathBuf {
        PathBuf::from(env::var_os("APPDATA").unwrap_or_default()).join("lumen").join("config.json")
    }

    pub fn load() -> Config {
        Config::load_from(&Config::path())
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to(&Config::path())
    }

    fn load_from(path: &Path) -> Config {
        fs::read(path).ok().and_then(|b| serde_json::from_slice::<Config>(&b).ok()).map(Config::sanitized).unwrap_or_default()
    }

    fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(d) = path.parent() {
            fs::create_dir_all(d)?;
        }
        fs::write(path, serde_json::to_vec_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        env::temp_dir().join(format!("lumen-config-{}-{name}.json", std::process::id()))
    }

    #[test]
    fn defaults_when_the_file_is_missing_or_broken() {
        assert_eq!(Config::load_from(&tmp("no-existe")), Config::default());
        let p = tmp("roto");
        fs::write(&p, "esto no es json").unwrap();
        assert_eq!(Config::load_from(&p), Config::default());
        let _ = fs::remove_file(p);
    }

    #[test]
    fn saves_and_loads_the_same_values() {
        let p = tmp("ida-y-vuelta");
        let c = Config { theme: "dark".into(), refresh_secs: 5, language: "en".into() };
        c.save_to(&p).unwrap();
        assert_eq!(Config::load_from(&p), c);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn invalid_values_fall_back_and_missing_fields_use_defaults() {
        let p = tmp("invalido");
        fs::write(&p, r#"{"theme":"rosa","refresh_secs":7,"language":"fr"}"#).unwrap();
        assert_eq!(Config::load_from(&p), Config::default());
        fs::write(&p, r#"{"theme":"light"}"#).unwrap(); // falta el resto: se completa con lo de siempre
        let c = Config::load_from(&p);
        assert_eq!((c.theme.as_str(), c.refresh_secs, c.language.as_str()), ("light", 1, "es"));
        let _ = fs::remove_file(p);
    }
}
