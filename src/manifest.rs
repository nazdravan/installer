//! Fișierul-descriere `program.toml` de lângă fiecare installer.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::system::{Arch, WinVer};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Msi,
    Exe,
    /// Program portabil: se dezarhivează în `target` și primește scurtături.
    Zip,
    Deb,
    /// Pe distribuțiile Debian se instalează prin `alien`.
    Rpm,
    /// Se dezarhivează în `target` (implicit `/opt/<nume>`), fără FUSE.
    #[serde(rename = "appimage")]
    AppImage,
}

impl Kind {
    pub fn is_linux(self) -> bool {
        matches!(self, Kind::Deb | Kind::Rpm | Kind::AppImage)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    /// Windows: începutul lui `DisplayName` din cheile `Uninstall` din registry.
    pub registry_name: Option<String>,
    /// Linux: numele exact al pachetului pentru `dpkg-query`.
    pub package: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    #[serde(default)]
    pub windows: Vec<WinVer>,
    pub arch: Arch,
    pub kind: Kind,
    pub file: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_success_codes")]
    pub success_codes: Vec<i32>,
    /// 3010: reușit, cere repornire; 1641: reușit, repornirea a fost pornită.
    #[serde(default = "default_reboot_codes")]
    pub reboot_codes: Vec<i32>,
    pub target: Option<String>,
    pub shortcut: Option<String>,
    #[serde(default = "yes")]
    pub desktop: bool,
    #[serde(default = "default_timeout_min")]
    pub timeout_min: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub name: String,
    pub version: String,
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "yes")]
    pub default: bool,
    pub detect: Option<Detect>,
    #[serde(rename = "variant", default)]
    pub variants: Vec<Variant>,
}

impl Variant {
    /// Mapa în care se instalează un program portabil (zip sau AppImage).
    pub fn target_dir(&self, program_name: &str) -> Option<PathBuf> {
        match (&self.target, self.kind) {
            (Some(t), _) => Some(PathBuf::from(t)),
            (None, Kind::AppImage) => Some(PathBuf::from(format!("/opt/{}", program_name.to_lowercase().replace(' ', "-")))),
            _ => None,
        }
    }
}

fn yes() -> bool {
    true
}
fn default_success_codes() -> Vec<i32> {
    vec![0]
}
fn default_reboot_codes() -> Vec<i32> {
    vec![3010, 1641]
}
fn default_timeout_min() -> u64 {
    30
}

/// Citește descrierea și verifică ce nu poate verifica TOML-ul: că fișierele există și că
/// fiecare tip are câmpurile de care are nevoie. `dir` este mapa programului.
pub fn parse(text: &str, dir: &Path) -> Result<Program, String> {
    let program: Program = toml::from_str(text).map_err(|e| romanian(e.message()))?;
    if program.variants.is_empty() {
        return Err("nu are nicio variantă ([[variant]])".into());
    }
    for v in &program.variants {
        if !dir.join(&v.file).is_file() {
            return Err(format!("lipsește fișierul „{}”", v.file));
        }
        match v.kind {
            Kind::Zip if v.target.is_none() || v.shortcut.is_none() => {
                return Err("varianta zip are nevoie de „target” și „shortcut”".into());
            }
            Kind::Msi | Kind::Exe if program.detect.as_ref().and_then(|d| d.registry_name.as_ref()).is_none() => {
                return Err("variantele msi și exe au nevoie de secțiunea [detect]".into());
            }
            Kind::Deb | Kind::Rpm if program.detect.as_ref().and_then(|d| d.package.as_ref()).is_none() => {
                return Err("variantele deb și rpm au nevoie de [detect] package".into());
            }
            _ => {}
        }
    }
    Ok(program)
}

/// Traduce mesajele frecvente ale lui serde; celelalte rămân cum sunt.
fn romanian(msg: &str) -> String {
    let quoted = |s: &str| s.replace('`', "");
    let name = |rest: &str| rest.split('`').nth(1).unwrap_or("").to_string();
    let expected = |rest: &str| rest.split_once("expected ").map(|(_, e)| quoted(e.trim_start_matches("one of ")));
    if let Some(rest) = msg.strip_prefix("missing field ") {
        format!("lipsește câmpul „{}”", name(rest))
    } else if let Some(rest) = msg.strip_prefix("unknown field ") {
        let mut out = format!("câmp necunoscut „{}”", name(rest));
        if let Some(e) = expected(rest) {
            out += &format!("; sunt permise: {e}");
        }
        out
    } else if let Some(rest) = msg.strip_prefix("unknown variant ") {
        let mut out = format!("valoare necunoscută „{}”", name(rest));
        if let Some(e) = expected(rest) {
            out += &format!("; sunt permise: {e}");
        }
        out
    } else {
        msg.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FURNICA: &str = r#"
name = "Furnica"
version = "3.0.4"
category = "Educațional"
default = true

[[variant]]
windows = ["8", "10"]
arch = "x86"
kind = "zip"
file = "furnica.zip"
target = 'C:\Program Files\Furnica'
shortcut = "Furnica.exe"
"#;

    fn dir_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for f in files {
            std::fs::write(dir.path().join(f), b"").unwrap();
        }
        dir
    }

    #[test]
    fn parses_furnica_example() {
        let dir = dir_with(&["furnica.zip"]);
        let p = parse(FURNICA, dir.path()).unwrap();
        let v = &p.variants[0];
        assert_eq!(v.kind, Kind::Zip);
        assert_eq!(v.windows, [WinVer::Win8, WinVer::Win10]);
        assert!(v.desktop);
        assert_eq!(v.timeout_min, 30);
        assert_eq!(v.success_codes, [0]);
        assert_eq!(v.target.as_deref(), Some(r"C:\Program Files\Furnica"));
    }

    #[test]
    fn msi_without_detect_is_error() {
        let dir = dir_with(&["a.msi"]);
        let text = "name = \"A\"\nversion = \"1\"\ncategory = \"B\"\n[[variant]]\nwindows = [\"10\"]\narch = \"x64\"\nkind = \"msi\"\nfile = \"a.msi\"\n";
        assert!(parse(text, dir.path()).unwrap_err().contains("[detect]"));
    }

    #[test]
    fn zip_without_target_is_error() {
        let dir = dir_with(&["furnica.zip"]);
        let no_target = FURNICA.replace("target = 'C:\\Program Files\\Furnica'\n", "");
        assert!(parse(&no_target, dir.path()).unwrap_err().contains("target"));
        let no_shortcut = FURNICA.replace("shortcut = \"Furnica.exe\"\n", "");
        assert!(parse(&no_shortcut, dir.path()).unwrap_err().contains("shortcut"));
    }

    #[test]
    fn missing_file_is_error() {
        let dir = dir_with(&[]);
        assert!(parse(FURNICA, dir.path()).unwrap_err().contains("furnica.zip"));
    }

    #[test]
    fn no_variants_is_error() {
        let dir = dir_with(&[]);
        let text = "name = \"A\"\nversion = \"1\"\ncategory = \"B\"\n";
        assert!(parse(text, dir.path()).unwrap_err().contains("variant"));
    }

    #[test]
    fn unknown_field_is_error() {
        let dir = dir_with(&["furnica.zip"]);
        let err = parse(&format!("nume = \"x\"\n{FURNICA}"), dir.path()).unwrap_err();
        assert!(err.starts_with("câmp necunoscut „nume”"), "{err}");
    }

    #[test]
    fn deb_without_package_is_error() {
        let dir = dir_with(&["a.deb"]);
        let text = "name = \"A\"\nversion = \"1\"\ncategory = \"B\"\n[detect]\nregistry_name = \"A\"\n[[variant]]\narch = \"x64\"\nkind = \"deb\"\nfile = \"a.deb\"\n";
        assert_eq!(parse(text, dir.path()).unwrap_err(), "variantele deb și rpm au nevoie de [detect] package");
        let ok = text.replace("registry_name = \"A\"", "package = \"a\"");
        assert!(parse(&ok, dir.path()).is_ok());
    }

    #[test]
    fn appimage_default_target() {
        let dir = dir_with(&["s.AppImage"]);
        let text = "name = \"Sumatra PDF\"\nversion = \"1\"\ncategory = \"B\"\n[[variant]]\narch = \"x64\"\nkind = \"appimage\"\nfile = \"s.AppImage\"\n";
        let p = parse(text, dir.path()).unwrap();
        assert_eq!(p.variants[0].kind, Kind::AppImage);
        assert!(p.variants[0].kind.is_linux());
        assert!(!Kind::Zip.is_linux());
        assert_eq!(p.variants[0].target_dir(&p.name), Some(PathBuf::from("/opt/sumatra-pdf")));
    }

    #[test]
    fn missing_field_is_romanian() {
        let dir = dir_with(&[]);
        assert_eq!(parse("name = \"X\"", dir.path()).unwrap_err(), "lipsește câmpul „version”");
    }
}
