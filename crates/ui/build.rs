// Manifiesto: administrador en la versión final, rutas largas y compatibilidad declarada con Windows 10 y 11.
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="LEVEL" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
</assembly>
"#;

fn main() {
    slint_build::compile_with_config("ui/main.slint", slint_build::CompilerConfiguration::new().with_bundled_translations("lang")).unwrap();

    // Ícono, nombre y versión del ejecutable: sin esto Windows lo muestra como un programa anónimo.
    // La versión (FileVersion/ProductVersion) sale de Cargo.toml.
    // La versión final pide administrador al abrir (cambia el registro, servicios y puntos de restauración).
    // En desarrollo (debug) se abre sin pedirlo para poder probar sin la ventana de permisos.
    let level = if std::env::var("PROFILE").is_ok_and(|p| p == "release") { "requireAdministrator" } else { "asInvoker" };
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico")
        .set("ProductName", "lumen")
        .set("FileDescription", "lumen, optimizador de PC liviano y honesto")
        .set("InternalName", "lumen")
        .set("OriginalFilename", "lumen.exe")
        .set("CompanyName", "luca vitorino")
        .set("LegalCopyright", "Copyright (c) 2026 luca vitorino. Licencia MIT")
        .set_manifest(&MANIFEST.replace("LEVEL", level));
    res.compile().unwrap();
}
