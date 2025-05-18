#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod catalog;
mod command;
mod detect;
mod exec;
mod journal;
mod manifest;
mod select;
#[cfg(windows)]
mod software_gl;
mod system;
mod version;

use std::path::PathBuf;

fn main() -> eframe::Result {
    #[cfg(windows)]
    let software = software_gl::enable_if_requested();

    // Totul se caută lângă Instalator.exe, oricare ar fi litera stick-ului.
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    // La dezvoltare se poate da altă mapă cu programe: `cargo run -- stick/Windows`.
    let root = std::env::args_os()
        .skip(1)
        .find(|a| !a.to_string_lossy().starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| exe_dir.join("Windows"));

    let sys = system::detect();
    #[cfg(windows)]
    let reg = detect::WinRegistry;
    #[cfg(not(windows))]
    let reg = NoRegistry;
    let rows = app::build_rows(catalog::scan(&root), &sys, &reg);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Instalator școală")
            .with_inner_size([760.0, 560.0])
            .with_min_inner_size([560.0, 400.0]),
        ..Default::default()
    };
    let journal_dir = exe_dir.join("jurnal");
    let result = eframe::run_native(
        "Instalator școală",
        options,
        Box::new(|_cc| Ok(Box::new(app::InstallerApp::new(sys, journal_dir, rows)))),
    );

    if let Err(e) = &result {
        // Fără placă video compatibilă: încearcă din nou cu OpenGL software (Mesa).
        #[cfg(windows)]
        if !software && software_gl::relaunch() {
            return Ok(());
        }
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Instalator școală")
            .set_description(format!(
                "Instalatorul nu poate porni: placa video nu suportă OpenGL 2.0.\n\
                 Puneți dosarul „soft-gl” lângă program.\n\n{e}"
            ))
            .show();
    }
    result
}

/// Pe macOS/Linux nu există registry: la dezvoltare toate programele msi/exe par neinstalate.
#[cfg(not(windows))]
struct NoRegistry;

#[cfg(not(windows))]
impl detect::Registry for NoRegistry {
    fn uninstall_entries(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}
