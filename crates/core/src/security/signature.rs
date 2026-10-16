//! Hash SHA-256 y firma digital de ejecutables.
//! La firma se verifica con `Get-AuthenticodeSignature`, que entiende también las firmas por
//! catálogo (la mayoría de los archivos de Windows): una verificación simple los marcaría como "sin firma".
use crate::ps;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, io::Read, path::{Path, PathBuf}};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Sig {
    /// Firma válida; guarda el nombre del editor.
    Valid(String),
    Unsigned,
    /// La firma existe pero no es confiable o no coincide con el archivo.
    Invalid(String),
    /// No se pudo determinar (formato sin firma posible, archivo ausente).
    #[default]
    Unknown,
}

impl Sig {
    pub fn is_valid(&self) -> bool {
        matches!(self, Sig::Valid(_))
    }
    pub fn label(&self) -> String {
        match self {
            Sig::Valid(p) => format!("firmado por {p}"),
            Sig::Unsigned => "sin firma".into(),
            Sig::Invalid(s) => format!("firma inválida ({s})"),
            Sig::Unknown => "firma desconocida".into(),
        }
    }
}

/// SHA-256 en hexadecimal. None si no se puede leer o pesa más de 256 MB.
pub fn sha256(path: &Path) -> Option<String> {
    if fs::metadata(path).ok()?.len() > 256 * 1024 * 1024 {
        return None;
    }
    let mut f = fs::File::open(path).ok()?;
    let (mut h, mut buf) = (Sha256::new(), vec![0u8; 64 * 1024]);
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Nombre del editor a partir del sujeto del certificado ("CN=Microsoft Windows, O=..." -> "Microsoft Windows").
pub fn publisher(subject: &str) -> String {
    subject
        .split(", ")
        .find_map(|p| p.strip_prefix("CN="))
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_else(|| subject.to_string())
}

fn classify(status: &str, subject: &str) -> Sig {
    match status {
        "Valid" => Sig::Valid(publisher(subject)),
        "NotSigned" => Sig::Unsigned,
        "NotSupportedFileFormat" | "" => Sig::Unknown,
        other => Sig::Invalid(other.to_string()),
    }
}

/// Firma de varios archivos. Cada consulta a PowerShell tiene un costo fijo y verificar es lento, así que
/// se reparte en hasta 4 consultas simultáneas. Las rutas viajan por un archivo temporal para no escaparlas.
pub fn signatures(paths: &[PathBuf]) -> HashMap<PathBuf, Sig> {
    if paths.is_empty() {
        return HashMap::new();
    }
    let chunk = paths.len().div_ceil(4).max(20);
    std::thread::scope(|s| {
        let jobs: Vec<_> = paths.chunks(chunk).enumerate().map(|(i, c)| s.spawn(move || signatures_chunk(c, i))).collect();
        jobs.into_iter().flat_map(|j| j.join().unwrap_or_default()).collect()
    })
}

fn signatures_chunk(paths: &[PathBuf], id: usize) -> HashMap<PathBuf, Sig> {
    let mut out = HashMap::new();
    let tmp = std::env::temp_dir().join(format!("lumen-paths-{}-{id}.txt", std::process::id()));
    let list: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    if fs::write(&tmp, list.join("\n")).is_err() {
        return out;
    }
    let script = format!(
        "Get-Content -LiteralPath {} -Encoding UTF8 | ForEach-Object {{ $s = Get-AuthenticodeSignature -LiteralPath $_ -ErrorAction SilentlyContinue; \
         [pscustomobject]@{{ path=[string]$_; status=[string]$s.Status; subject=[string]$s.SignerCertificate.Subject }} }} | ConvertTo-Json -Compress",
        ps::quote(&tmp.to_string_lossy())
    );
    let json = ps::run(&script).unwrap_or_default();
    let _ = fs::remove_file(&tmp);
    for r in ps::rows(&json) {
        out.insert(PathBuf::from(ps::text(&r, "path")), classify(&ps::text(&r, "status"), &ps::text(&r, "subject")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publisher_takes_the_common_name() {
        assert_eq!(publisher(r#"CN=Microsoft Windows, O=Microsoft Corporation, L=Redmond, S=Washington, C=US"#), "Microsoft Windows");
        assert_eq!(publisher(r#"CN="Valve Corp.", O=Valve, C=US"#), "Valve Corp.");
        assert_eq!(publisher("sin cn"), "sin cn");
    }

    #[test]
    fn classifies_statuses() {
        assert_eq!(classify("Valid", "CN=X, O=Y"), Sig::Valid("X".into()));
        assert_eq!(classify("NotSigned", ""), Sig::Unsigned);
        assert_eq!(classify("HashMismatch", ""), Sig::Invalid("HashMismatch".into()));
        assert_eq!(classify("", ""), Sig::Unknown);
    }

    #[test]
    fn hashes_known_content() {
        let f = std::env::temp_dir().join(format!("lumen-sha-{}.txt", std::process::id()));
        fs::write(&f, "abc").unwrap();
        // SHA-256 de "abc" (vector de prueba estándar)
        assert_eq!(sha256(&f).as_deref(), Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
        let _ = fs::remove_file(f);
        assert!(sha256(Path::new(r"C:\no\existe.exe")).is_none());
    }

    #[test]
    fn real_windows_binary_is_valid_via_catalog() {
        // notepad.exe se firma por catálogo en muchas versiones: no debe salir como "sin firma".
        let p = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into())).join(r"System32\notepad.exe");
        let sigs = signatures(std::slice::from_ref(&p));
        assert!(sigs.get(&p).is_some_and(Sig::is_valid), "{:?}", sigs.get(&p));
    }
}
