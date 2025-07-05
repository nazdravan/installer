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

use std::path::{Path, PathBuf};

fn main() -> eframe::Result {
    #[cfg(windows)]
    let software = software_gl::enable_if_requested();

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    let arg = std::env::args_os().skip(1).find(|a| !a.to_string_lossy().starts_with("--")).map(PathBuf::from);
    let (root, journal_dir) = paths(&exe_dir, arg);

    let sys = system::detect();
    #[cfg(windows)]
    let reg = detect::WinRegistry;
    #[cfg(target_os = "linux")]
    let reg = detect::Dpkg;
    #[cfg(not(any(windows, target_os = "linux")))]
    let reg = NoRegistry;
    let rows = app::build_rows(catalog::scan(&root), &sys, &reg);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Instalator școală")
            .with_inner_size([760.0, 560.0])
            .with_min_inner_size([560.0, 400.0]),
        ..Default::default()
    };
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

/// Mapa cu programe și mapa jurnalului. Pe Windows ele sunt lângă Instalator.exe, oricare ar fi
/// litera stick-ului. Pe Linux programul rulează dintr-o copie din /tmp (stick-urile FAT32 nu
/// permit execuția), așa că `instalator.sh` dă mapa `Linux` ca argument; la dezvoltare la fel:
/// `cargo run -- stick/Windows`. Jurnalul stă lângă mapa cu programe.
fn paths(exe_dir: &Path, arg: Option<PathBuf>) -> (PathBuf, PathBuf) {
    let folder = if cfg!(target_os = "linux") { "Linux" } else { "Windows" };
    let root = arg.unwrap_or_else(|| exe_dir.join(folder));
    let journal = root.parent().unwrap_or(exe_dir).join("jurnal");
    (root, journal)
}

/// Pe macOS nu există registry: la dezvoltare toate programele msi/exe par neinstalate.
#[cfg(not(any(windows, target_os = "linux")))]
struct NoRegistry;

#[cfg(not(any(windows, target_os = "linux")))]
impl detect::Registry for NoRegistry {
    fn uninstall_entries(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_from_root() {
        let (root, journal) = paths(Path::new("/tmp/x"), Some("/media/s/Linux".into()));
        assert_eq!(root, Path::new("/media/s/Linux"));
        assert_eq!(journal, Path::new("/media/s/jurnal"));

        let (root, journal) = paths(Path::new("/media/s"), None);
        let folder = if cfg!(target_os = "linux") { "Linux" } else { "Windows" };
        assert_eq!(root, Path::new("/media/s").join(folder));
        assert_eq!(journal, Path::new("/media/s/jurnal"));
    }
}
