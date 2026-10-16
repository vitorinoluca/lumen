//! Opciones de la sección Rendimiento: plan de energía, modo juego, apps de inicio, servicios,
//! efectos visuales y apps en segundo plano. Todo se aplica con acciones del journal.
use crate::{
    actions::{Action, Hive},
    icons, power, profiles,
    plan::{Item, Risk, Step},
    services::{self, Level},
    startup,
};

const EXPLORER_ADV: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const BACKGROUND: &str = r"Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications";

fn step(action: Action) -> Vec<Step> {
    vec![Step { action }]
}

/// Planes de energía para elegir uno: (id, nombre, qué hace, GUID base).
const PLANS: &[(&str, &str, &str, &str)] = &[
    ("plan:balanced", "Equilibrado", "El plan normal de Windows: rinde cuando hace falta y ahorra cuando no.", power::BALANCED),
    ("plan:high", "Alto rendimiento", "La PC no baja la velocidad del procesador. Gasta más energía y calienta más.", power::HIGH_PERFORMANCE),
    ("plan:saver", "Ahorro de energía", "Reduce el rendimiento para que la batería dure más.", power::POWER_SAVER),
];

pub fn items() -> Vec<Item> {
    let mut v = vec![];

    // Plan de energía: se elige uno solo. El plan "alto rendimiento" se crea al aplicar si esta PC no lo trae.
    v.push(Item::header("Plan de energía"));
    let active = power::active().unwrap_or_default();
    for (id, name, desc, guid) in PLANS {
        let prev = active.clone();
        let mut it = Item::new(id, name, desc, Risk::Review, step(Action::SetPowerPlan { new: (*guid).into(), prev }));
        it.group = 1;
        it.done = &active == guid;
        v.push(it);
    }

    // Modo juego: alto rendimiento y cerrar las apps de la lista (la lista se edita en %APPDATA%\lumen\game-list.json).
    v.push(Item::header("Modo juego"));
    let list = profiles::game_list();
    let mut game = Item::new(
        "game",
        "Activar el modo juego",
        &format!("Alto rendimiento y cierra {} apps de tu lista (game-list.json). Lo cerrado no se reabre solo.", list.len()),
        Risk::Review,
        vec![Step { action: Action::SetPowerPlan { new: power::HIGH_PERFORMANCE.into(), prev: active.clone() } }, Step { action: Action::CloseProcesses { names: list } }],
    );
    game.done = active == power::HIGH_PERFORMANCE;
    v.push(game);

    // Apps de inicio: elegir cuáles no arrancan con Windows. Solo vos sabés cuáles necesitás: vienen desmarcadas.
    let entries = startup::list();
    if !entries.is_empty() {
        v.push(Item::header("Apps que arrancan con Windows"));
        for e in entries {
            let mut it = Item::new(
                &format!("startup:{}", e.name),
                &format!("No iniciar {} con Windows", e.name),
                &e.command,
                Risk::Review,
                step(Action::SetStartup { source: e.source, name: e.name.clone(), enable: false }),
            );
            it.done = !e.enabled;
            it.icon = icons::exe_of_command(&e.command).unwrap_or_default();
            v.push(it);
        }
    }

    // Servicios que casi nadie usa: los seguros vienen marcados; los que dependen del uso van en "Más opciones".
    v.push(Item::header("Servicios que casi nadie usa"));
    let mut caution = vec![];
    for s in services::list().into_iter().filter(|s| s.level != Level::Critical) {
        let Some(action) = services::disable_action(&s.name) else { continue };
        let safe = s.level == Level::Safe;
        let mut it = Item::new(&format!("service:{}", s.name), &s.title, &s.desc, if safe { Risk::Safe } else { Risk::Review }, step(action));
        it.done = !s.enabled();
        it.more = !safe;
        if safe { v.push(it) } else { caution.push(it) }
    }
    if !caution.is_empty() {
        v.push(Item { more: true, ..Item::header("Avanzado: pueden cambiar cómo usás la PC") });
        v.extend(caution);
    }

    // Efectos visuales: menos animaciones y transparencias. Cambian el aspecto, así que vienen desmarcados.
    v.push(Item::header("Efectos visuales"));
    let mut fx = |id: &str, title: &str, desc: &str, action: Action, done: bool| {
        let mut it = Item::new(id, title, desc, Risk::Review, step(action));
        it.done = done;
        v.push(it);
    };
    let min_anim = Action::set_text(Hive::User, r"Control Panel\Desktop\WindowMetrics", "MinAnimate", Some("0".into()));
    fx("fx:minanimate", "Quitar la animación al minimizar y maximizar", "Las ventanas aparecen y desaparecen al instante.", min_anim.clone(), is_text_off(&min_anim));
    let taskbar = Action::set_dword(Hive::User, EXPLORER_ADV, "TaskbarAnimations", 0);
    fx("fx:taskbar", "Quitar las animaciones de la barra de tareas", "Los botones de la barra no se deslizan ni se animan.", taskbar.clone(), is_dword(&taskbar, 0));
    let transp = Action::set_dword(Hive::User, PERSONALIZE, "EnableTransparency", 0);
    fx("fx:transparency", "Desactivar las transparencias de Windows", "Los menús y la barra dejan de ser translúcidos. Ahorra algo de trabajo a la tarjeta gráfica.", transp.clone(), is_dword(&transp, 0));

    // Segundo plano: evita que las apps trabajen y avisen cuando no las usás.
    v.push(Item::header("Segundo plano"));
    let bg = Action::set_dword(Hive::User, BACKGROUND, "GlobalUserDisabled", 1);
    let mut it = Item::new("background", "No dejar que las apps trabajen en segundo plano", "Las apps de la Tienda no se actualizan ni te avisan cuando están cerradas.", Risk::Review, step(bg.clone()));
    it.done = is_dword(&bg, 1);
    v.push(it);
    v
}

/// ¿Ya vale el valor que pondría esta acción?
fn is_dword(action: &Action, want: u32) -> bool {
    match action {
        Action::SetDword { prev, .. } => *prev == Some(want),
        _ => false,
    }
}

fn is_text_off(action: &Action) -> bool {
    matches!(action, Action::SetRegText { new, prev, .. } if prev == new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_are_a_single_choice_and_everything_risky_starts_unchecked() {
        let items = items();
        assert!(items.iter().filter(|i| i.group == 1).count() == PLANS.len());
        assert!(items.iter().filter(|i| i.risk == Risk::Review).all(|i| !i.default_checked()));
        // Los servicios críticos nunca llegan a la lista.
        assert!(!items.iter().any(|i| i.id == "service:RpcSs"));
    }
}
