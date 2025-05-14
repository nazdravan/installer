//! Ce versiune a unui program e deja instalată pe calculator.

use std::path::Path;

use crate::manifest::{Kind, Program, Variant};
use crate::version;

/// Fișierul lăsat de instalator în mapa unui program portabil, cu versiunea instalată.
pub const MARKER: &str = ".instalator-versiune";

/// Perechile (`DisplayName`, `DisplayVersion`) din cheile `Uninstall`.
pub trait Registry {
    fn uninstall_entries(&self) -> Vec<(String, String)>;
}

pub fn installed_version(p: &Program, v: &Variant, reg: &dyn Registry) -> Option<String> {
    if v.kind == Kind::Zip {
        let marker = Path::new(v.target.as_deref()?).join(MARKER);
        return std::fs::read_to_string(marker).ok().map(|s| s.trim().to_string());
    }
    let prefix = p.detect.as_ref()?.registry_name.to_lowercase();
    reg.uninstall_entries()
        .into_iter()
        .filter(|(name, _)| name.to_lowercase().starts_with(&prefix))
        .map(|(_, version)| version)
        .max_by(|a, b| version::compare(a, b))
}

#[cfg(windows)]
pub struct WinRegistry;

#[cfg(windows)]
impl Registry for WinRegistry {
    fn uninstall_entries(&self) -> Vec<(String, String)> {
        use winreg::RegKey;
        use winreg::enums::*;

        const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
        // Instalatorul e pe 32 de biți: fără KEY_WOW64_64KEY ar vedea doar WOW6432Node.
        let views = [
            (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
            (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
            (HKEY_CURRENT_USER, 0),
        ];
        let mut found = Vec::new();
        for (root, view) in views {
            let Ok(key) = RegKey::predef(root).open_subkey_with_flags(UNINSTALL, KEY_READ | view) else { continue };
            for name in key.enum_keys().flatten() {
                let Ok(app) = key.open_subkey_with_flags(&name, KEY_READ | view) else { continue };
                if let Ok(display) = app.get_value::<String, _>("DisplayName") {
                    found.push((display, app.get_value("DisplayVersion").unwrap_or_default()));
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeRegistry(Vec<(&'static str, &'static str)>);

    impl Registry for FakeRegistry {
        fn uninstall_entries(&self) -> Vec<(String, String)> {
            self.0.iter().map(|(n, v)| (n.to_string(), v.to_string())).collect()
        }
    }

    fn program(kind: &str, target: &str) -> Program {
        toml::from_str(&format!(
            "name = \"P\"\nversion = \"1\"\ncategory = \"C\"\n[detect]\nregistry_name = \"VLC media player\"\n[[variant]]\nwindows = [\"10\"]\narch = \"x86\"\nkind = \"{kind}\"\nfile = \"f\"\ntarget = '{target}'\nshortcut = \"x.exe\"\n"
        ))
        .unwrap()
    }

    #[test]
    fn registry_picks_highest_match() {
        let p = program("exe", "");
        let reg = FakeRegistry(vec![
            ("VLC media player", "3.0.18"),
            ("vlc media player", "3.0.21"),
            ("VLCx", "9"),
        ]);
        assert_eq!(installed_version(&p, &p.variants[0], &reg).as_deref(), Some("3.0.21"));
    }

    #[test]
    fn registry_no_match_is_none() {
        let p = program("msi", "");
        let reg = FakeRegistry(vec![("Altceva", "1")]);
        assert_eq!(installed_version(&p, &p.variants[0], &reg), None);
    }

    #[test]
    fn zip_reads_marker() {
        let dir = tempfile::tempdir().unwrap();
        let p = program("zip", dir.path().to_str().unwrap());
        let reg = FakeRegistry(vec![]);
        assert_eq!(installed_version(&p, &p.variants[0], &reg), None);
        std::fs::write(dir.path().join(MARKER), "3.0.2\n").unwrap();
        assert_eq!(installed_version(&p, &p.variants[0], &reg).as_deref(), Some("3.0.2"));
    }
}
