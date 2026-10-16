#![windows_subsystem = "windows"]
slint::include_modules!();

fn main() {
    Main::new().unwrap().run().unwrap();
}
