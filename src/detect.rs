//! Ce versiune a unui program e deja instalată pe calculator.

use crate::manifest::{Kind, Program, Variant};
use crate::version;

/// Fișierul lăsat de instalator în mapa unui program portabil, cu versiunea instalată.
pub const MARKER: &str = ".instalator-versiune";

/// Perechile (`DisplayName`, `DisplayVersion`) din cheile `Uninstall`.
pub trait Registry {
    fn uninstall_entries(&self) -> Vec<(String, String)>;

    /// Linux: versiunea pachetului instalat, cum o dă dpkg.
    fn package_version(&self, _package: &str) -> Option<String> {
        None
    }
}

/// Versiunea Debian fără epoch (`1:2.3-1ubuntu2` → `2.3-1ubuntu2`); revizia rămâne și contează
/// doar ca segmente în plus la comparare.
pub fn debian_version(v: &str) -> String {
    v.split_once(':').map_or(v, |(_, rest)| rest).to_string()
}

pub fn installed_version(p: &Program, v: &Variant, reg: &dyn Registry) -> Option<String> {
    match v.kind {
        Kind::Zip | Kind::AppImage => {
            let marker = v.target_dir(&p.name)?.join(MARKER);
            return std::fs::read_to_string(marker).ok().map(|s| s.trim().to_string());
        }
        Kind::Deb | Kind::Rpm => {
            // Un rpm instalat prin alien devine tot un pachet dpkg.
            let package = p.detect.as_ref()?.package.as_deref()?;
            return reg.package_version(package).map(|v| debian_version(&v));
        }
        Kind::Msi | Kind::Exe => {}
    }
    let prefix = p.detect.as_ref()?.registry_name.as_ref()?.to_lowercase();
    reg.uninstall_entries()
        .into_iter()
        .filter(|(name, _)| name.to_lowercase().starts_with(&prefix))
        .map(|(_, version)| version)
        .max_by(|a, b| version::compare(a, b))
}

#[cfg(target_os = "linux")]
pub struct Dpkg;

#[cfg(target_os = "linux")]
impl Registry for Dpkg {
    fn uninstall_entries(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn package_version(&self, package: &str) -> Option<String> {
        let out = std::process::Command::new("dpkg-query")
            .args(["-W", "-f", "${Status}\t${Version}", package])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let (status, version) = text.split_once('\t')?;
        // „deinstall ok config-files”: pachetul a fost șters, au rămas doar setările.
        (out.status.success() && status.ends_with(" installed")).then(|| version.trim().to_string())
    }
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

        fn package_version(&self, package: &str) -> Option<String> {
            self.0.iter().find(|(n, _)| *n == package).map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn dpkg_version_strips_epoch() {
        assert_eq!(debian_version("1:2.3-1ubuntu2"), "2.3-1ubuntu2");
        assert_eq!(debian_version("3.0.5"), "3.0.5");
        assert!(version::compare(&debian_version("1:2.3-1ubuntu2"), "2.3").is_ge());
    }

    #[test]
    fn deb_uses_package_version() {
        let p: Program = toml::from_str(
            "name = \"Furnica\"\nversion = \"3.0.5\"\ncategory = \"C\"\n[detect]\npackage = \"furnica\"\n[[variant]]\narch = \"x64\"\nkind = \"deb\"\nfile = \"f.deb\"\n",
        )
        .unwrap();
        let reg = FakeRegistry(vec![("furnica-data", "9"), ("furnica", "1:3.0.4")]);
        assert_eq!(installed_version(&p, &p.variants[0], &reg).as_deref(), Some("3.0.4"));
        assert_eq!(installed_version(&p, &p.variants[0], &FakeRegistry(vec![])), None);
    }

    #[test]
    fn appimage_reads_marker() {
        let dir = tempfile::tempdir().unwrap();
        let p: Program = toml::from_str(&format!(
            "name = \"P\"\nversion = \"1\"\ncategory = \"C\"\n[[variant]]\narch = \"x64\"\nkind = \"appimage\"\nfile = \"p.AppImage\"\ntarget = '{}'\n",
            dir.path().display()
        ))
        .unwrap();
        std::fs::write(dir.path().join(MARKER), "0.9").unwrap();
        assert_eq!(installed_version(&p, &p.variants[0], &FakeRegistry(vec![])).as_deref(), Some("0.9"));
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
