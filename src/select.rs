//! Ce variantă se potrivește calculatorului și ce trebuie făcut cu fiecare program.

use crate::manifest::{Kind, Program, Variant};
use crate::system::{Arch, SystemInfo};
use crate::version;

/// Întâi o variantă exactă; pe 64 de biți merge și una de 32.
pub fn choose_variant<'a>(p: &'a Program, sys: &SystemInfo) -> Option<&'a Variant> {
    if sys.linux.is_some() {
        // Pe distribuțiile Debian: întâi pachetul nativ, apoi AppImage, rpm doar prin alien.
        let fits = |v: &&Variant| v.kind.is_linux() && (v.arch == sys.arch || v.arch == Arch::X86);
        return [Kind::Deb, Kind::AppImage, Kind::Rpm]
            .into_iter()
            .find_map(|kind| p.variants.iter().filter(fits).find(|v| v.kind == kind && v.arch == sys.arch)
                .or_else(|| p.variants.iter().filter(fits).find(|v| v.kind == kind)));
    }
    let win = sys.win?;
    let fits = |v: &&Variant, arch| !v.kind.is_linux() && v.windows.contains(&win) && v.arch == arch;
    p.variants
        .iter()
        .find(|v| fits(v, sys.arch))
        .or_else(|| (sys.arch == Arch::X64).then(|| p.variants.iter().find(|v| fits(v, Arch::X86))).flatten())
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Missing,
    Outdated { installed: String },
    UpToDate { installed: String },
    Unsupported(String),
    Invalid(String),
}

pub fn status(p: &Result<Program, String>, sys: &SystemInfo, installed: Option<&str>) -> Status {
    let p = match p {
        Ok(p) => p,
        Err(e) => return Status::Invalid(e.clone()),
    };
    if choose_variant(p, sys).is_none() {
        let os = match (&sys.linux, sys.win) {
            (Some(_), _) => "Linux",
            (None, Some(win)) => win.name(),
            (None, None) => return Status::Unsupported("Windows nesuportat".into()),
        };
        return Status::Unsupported(format!("nu există variantă pentru {os} ({})", sys.arch.bits()));
    }
    match installed {
        None => Status::Missing,
        Some(v) if version::compare(v, &p.version).is_lt() => Status::Outdated { installed: v.into() },
        Some(v) => Status::UpToDate { installed: v.into() },
    }
}

impl Status {
    /// Textul gri de lângă numele programului; `p_version` e versiunea de pe stick.
    pub fn label(&self, p_version: &str) -> String {
        match self {
            Status::Missing => "nu e instalat".into(),
            Status::Outdated { installed } => format!("actualizare {installed} ➡ {p_version}"),
            Status::UpToDate { installed } => format!("la zi ({installed})"),
            Status::Unsupported(why) | Status::Invalid(why) => why.clone(),
        }
    }

    pub fn checkable(&self) -> bool {
        !matches!(self, Status::Unsupported(_) | Status::Invalid(_))
    }

    pub fn default_checked(&self, program_default: bool) -> bool {
        match self {
            Status::Missing => program_default,
            Status::Outdated { .. } => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::WinVer;

    fn program(variants: &[(&str, &str)], default: bool) -> Program {
        let mut text = format!("name = \"P\"\nversion = \"3.0.4\"\ncategory = \"C\"\ndefault = {default}\n");
        for (windows, arch) in variants {
            text += &format!(
                "[[variant]]\nwindows = {windows}\narch = \"{arch}\"\nkind = \"zip\"\nfile = \"{arch}.zip\"\ntarget = 'C:\\x'\nshortcut = \"x.exe\"\n"
            );
        }
        toml::from_str(&text).unwrap()
    }

    fn sys(win: Option<WinVer>, arch: Arch) -> SystemInfo {
        SystemInfo { win, arch, admin: true, computer: "PC".into(), linux: None }
    }

    #[test]
    fn choose_exact_then_x86_fallback() {
        let only_x86 = program(&[(r#"["8", "10"]"#, "x86")], true);
        assert_eq!(choose_variant(&only_x86, &sys(Some(WinVer::Win10), Arch::X64)).unwrap().file, "x86.zip");
        assert_eq!(choose_variant(&only_x86, &sys(Some(WinVer::Win8), Arch::X86)).unwrap().file, "x86.zip");

        let both = program(&[(r#"["10"]"#, "x86"), (r#"["10"]"#, "x64")], true);
        assert_eq!(choose_variant(&both, &sys(Some(WinVer::Win10), Arch::X64)).unwrap().file, "x64.zip");
        assert_eq!(choose_variant(&both, &sys(Some(WinVer::Win10), Arch::X86)).unwrap().file, "x86.zip");
        assert!(choose_variant(&both, &sys(Some(WinVer::Win8), Arch::X64)).is_none());
    }

    #[test]
    fn x64_only_on_x86_is_unsupported() {
        let p = Ok(program(&[(r#"["10"]"#, "x64")], true));
        let s = status(&p, &sys(Some(WinVer::Win10), Arch::X86), None);
        assert_eq!(s.label("3.0.4"), "nu există variantă pentru Windows 10 (32 biți)");
        assert!(!s.checkable());
        assert!(!s.default_checked(true));
    }

    #[test]
    fn status_outdated_uptodate_missing() {
        let p = Ok(program(&[(r#"["10"]"#, "x86")], true));
        let s10 = sys(Some(WinVer::Win10), Arch::X64);

        let s = status(&p, &s10, Some("3.0.2"));
        assert_eq!(s, Status::Outdated { installed: "3.0.2".into() });
        assert_eq!(s.label("3.0.4"), "actualizare 3.0.2 ➡ 3.0.4");
        assert!(s.default_checked(false));

        let s = status(&p, &s10, Some("3.0.4"));
        assert_eq!(s.label("3.0.4"), "la zi (3.0.4)");
        assert!(!s.default_checked(true));
        assert!(s.checkable());

        assert!(matches!(status(&p, &s10, Some("3.1")), Status::UpToDate { .. }));

        let s = status(&p, &s10, None);
        assert_eq!(s, Status::Missing);
        assert_eq!(s.label("3.0.4"), "nu e instalat");
        assert!(!s.default_checked(false));
        assert!(s.default_checked(true));
    }

    fn linux_program(kinds: &[&str]) -> Program {
        let mut text = "name = \"P\"\nversion = \"1\"\ncategory = \"C\"\n[detect]\npackage = \"p\"\n".to_string();
        for kind in kinds {
            text += &format!("[[variant]]\narch = \"x64\"\nkind = \"{kind}\"\nfile = \"p.{kind}\"\n");
        }
        toml::from_str(&text).unwrap()
    }

    fn linux() -> SystemInfo {
        SystemInfo { win: None, arch: Arch::X64, admin: true, computer: "PC".into(), linux: Some("Ubuntu".into()) }
    }

    #[test]
    fn linux_prefers_deb_then_appimage() {
        let p = linux_program(&["rpm", "appimage", "deb"]);
        assert_eq!(choose_variant(&p, &linux()).unwrap().file, "p.deb");
        let p = linux_program(&["rpm", "appimage"]);
        assert_eq!(choose_variant(&p, &linux()).unwrap().file, "p.appimage");
        let p = linux_program(&["rpm"]);
        assert_eq!(choose_variant(&p, &linux()).unwrap().file, "p.rpm");
    }

    #[test]
    fn windows_ignores_linux_variants() {
        let p = linux_program(&["deb"]);
        assert!(choose_variant(&p, &sys(Some(WinVer::Win10), Arch::X64)).is_none());
    }

    #[test]
    fn linux_without_variant_label() {
        let p = Ok(program(&[(r#"["10"]"#, "x86")], true));
        assert_eq!(status(&p, &linux(), None).label("1"), "nu există variantă pentru Linux (64 biți)");
    }

    #[test]
    fn unsupported_windows() {
        let p = Ok(program(&[(r#"["10"]"#, "x86")], true));
        let s = status(&p, &sys(None, Arch::X64), None);
        assert_eq!(s.label("3.0.4"), "Windows nesuportat");
        assert!(!s.checkable());
    }

    #[test]
    fn invalid_shows_error() {
        let s = status(&Err("lipsește program.toml".into()), &sys(Some(WinVer::Win10), Arch::X64), None);
        assert_eq!(s.label(""), "lipsește program.toml");
        assert!(!s.checkable());
    }
}
