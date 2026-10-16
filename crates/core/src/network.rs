//! Opciones de la sección Red: limpiar la caché de DNS, restablecer la red y elegir el servidor DNS.
//! Cambiar de DNS guarda el valor de cada adaptador y se puede deshacer desde Historial.
use crate::{
    actions::Action,
    plan::{Item, Risk, Step},
    ps,
    tasks::Task,
};
use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

/// Servidores DNS para elegir: (id, nombre, qué es, direcciones). Sin direcciones = automático.
const DNS: &[(&str, &str, &str, &[&str])] = &[
    ("dns:auto", "DNS automático", "Usa el que te da tu proveedor de internet.", &[]),
    ("dns:cloudflare", "DNS de Cloudflare (1.1.1.1)", "Rápido y que no guarda tu actividad.", &["1.1.1.1", "1.0.0.1"]),
    ("dns:google", "DNS de Google (8.8.8.8)", "Muy estable y disponible en todo el mundo.", &["8.8.8.8", "8.8.4.4"]),
];

/// Adaptadores activos y sus DNS fijos. Vacío = automático (los da el router).
pub fn dns_state() -> Vec<(String, Vec<String>)> {
    let json = ps::run("Get-NetAdapter | Where-Object Status -eq 'Up' | ForEach-Object { [pscustomobject]@{ alias = $_.Name; guid = [string]$_.InterfaceGuid } } | ConvertTo-Json -Compress").unwrap_or_default();
    ps::rows(&json)
        .iter()
        .map(|a| {
            let key = format!(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{}", ps::text(a, "guid"));
            let fixed: String = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(key).and_then(|k| k.get_value("NameServer")).unwrap_or_default();
            (ps::text(a, "alias"), fixed.split([',', ' ']).filter(|s| !s.is_empty()).map(String::from).collect())
        })
        .collect()
}

fn task(id: &str, title: &str, desc: &str, risk: Risk, task: Task) -> Item {
    Item::new(id, title, desc, risk, vec![Step { action: Action::Maintenance { task } }])
}

pub fn items() -> Vec<Item> {
    let mut v = vec![
        task("net:flush", "Limpiar la caché de DNS", "Borra las direcciones que Windows recuerda. Sirve si una página no carga o muestra una versión vieja.", Risk::Safe, Task::FlushDns),
        task("net:reset", "Restablecer la configuración de red", "Arregla problemas de conexión raros. Hay que reiniciar la PC y no se puede deshacer.", Risk::Review, Task::ResetNetwork),
    ];
    let mut reset = v.pop().unwrap();
    reset.more = true;
    v.push(reset);

    v.push(Item::header("Servidor DNS"));
    let state = dns_state();
    for (id, name, desc, servers) in DNS {
        let servers: Vec<String> = servers.iter().map(|s| s.to_string()).collect();
        let risk = if servers.is_empty() { Risk::Safe } else { Risk::Review };
        let mut it = Item::new(id, name, desc, risk, vec![Step { action: Action::SetDns { servers: servers.clone(), prev: state.clone() } }]);
        it.group = 1;
        it.done = !state.is_empty() && state.iter().all(|(_, s)| *s == servers);
        v.push(it);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_choices_are_a_single_choice_and_the_reset_is_hidden_and_unchecked() {
        let items = items();
        assert_eq!(items.iter().filter(|i| i.group == 1).count(), DNS.len());
        let reset = items.iter().find(|i| i.id == "net:reset").unwrap();
        assert!(reset.more && !reset.default_checked());
        assert!(items.iter().find(|i| i.id == "net:flush").unwrap().default_checked());
    }
}
