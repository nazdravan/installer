//! Ce trebuie rulat pentru a instala o variantă.

use std::path::{Path, PathBuf};

use crate::manifest::{Kind, Program, Variant};

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Run { exe: PathBuf, args: Vec<String> },
    Unzip { zip: PathBuf, target: PathBuf, shortcut_exe: String, name: String, desktop: bool, version: String },
}

/// `folder` este mapa programului de pe stick.
pub fn action_for(p: &Program, v: &Variant, folder: &Path) -> Action {
    let file = folder.join(&v.file);
    match v.kind {
        Kind::Msi => {
            let mut args = vec!["/i".into(), file.display().to_string(), "/qn".into(), "/norestart".into()];
            args.extend(v.args.iter().cloned());
            Action::Run { exe: "msiexec".into(), args }
        }
        Kind::Exe => Action::Run { exe: file, args: v.args.clone() },
        // `parse` a verificat deja că target și shortcut există.
        Kind::Zip => Action::Unzip {
            zip: file,
            target: v.target.clone().unwrap_or_default().into(),
            shortcut_exe: v.shortcut.clone().unwrap_or_default(),
            name: p.name.clone(),
            desktop: v.desktop,
            version: p.version.clone(),
        },
    }
}

impl Action {
    /// O linie pentru jurnal.
    pub fn describe(&self) -> String {
        match self {
            Action::Run { exe, args } => std::iter::once(exe.display().to_string())
                .chain(args.iter().cloned())
                .map(|a| format!("\"{a}\""))
                .collect::<Vec<_>>()
                .join(" "),
            Action::Unzip { zip, target, .. } => format!("dezarhivare {} → {}", zip.display(), target.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(kind: &str, file: &str, args: &str) -> Program {
        toml::from_str(&format!(
            "name = \"Furnica\"\nversion = \"3.0.4\"\ncategory = \"C\"\n[detect]\nregistry_name = \"x\"\n[[variant]]\nwindows = [\"10\"]\narch = \"x86\"\nkind = \"{kind}\"\nfile = \"{file}\"\nargs = {args}\ntarget = 'C:\\Program Files\\Furnica'\nshortcut = \"Furnica.exe\"\n"
        ))
        .unwrap()
    }

    #[test]
    fn msi_command() {
        let p = program("msi", "a.msi", r#"["DESKTOPSHORTCUT="]"#);
        let msi = Path::new("F").join("a.msi").display().to_string();
        let a = action_for(&p, &p.variants[0], Path::new("F"));
        assert_eq!(
            a,
            Action::Run {
                exe: "msiexec".into(),
                args: ["/i", &msi, "/qn", "/norestart", "DESKTOPSHORTCUT="].map(String::from).to_vec(),
            }
        );
        assert_eq!(a.describe(), format!(r#""msiexec" "/i" "{msi}" "/qn" "/norestart" "DESKTOPSHORTCUT=""#));
    }

    #[test]
    fn exe_command() {
        let p = program("exe", "setup.exe", r#"["/S"]"#);
        assert_eq!(
            action_for(&p, &p.variants[0], Path::new("F")),
            Action::Run { exe: Path::new("F").join("setup.exe"), args: vec!["/S".into()] }
        );
    }

    #[test]
    fn zip_action() {
        let p = program("zip", "furnica.zip", "[]");
        let a = action_for(&p, &p.variants[0], Path::new("F"));
        assert_eq!(
            a,
            Action::Unzip {
                zip: Path::new("F").join("furnica.zip"),
                target: r"C:\Program Files\Furnica".into(),
                shortcut_exe: "Furnica.exe".into(),
                name: "Furnica".into(),
                desktop: true,
                version: "3.0.4".into(),
            }
        );
        let zip = Path::new("F").join("furnica.zip");
        assert_eq!(a.describe(), format!(r"dezarhivare {} → C:\Program Files\Furnica", zip.display()));
    }
}
