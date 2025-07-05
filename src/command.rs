//! Ce trebuie rulat pentru a instala o variantă.

use std::path::{Path, PathBuf};

use crate::manifest::{Kind, Program, Variant};

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Run { exe: PathBuf, args: Vec<String> },
    Unzip { zip: PathBuf, target: PathBuf, shortcut_exe: String, name: String, desktop: bool, version: String },
    AppImage { file: PathBuf, target: PathBuf, name: String, version: String },
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
        Kind::Deb => {
            // apt-get instalează un fișier local doar dacă primește o cale (cu „/”).
            let mut args: Vec<String> = ["DEBIAN_FRONTEND=noninteractive", "apt-get", "install", "-y"].map(String::from).to_vec();
            args.push(absolute(&file));
            Action::Run { exe: "env".into(), args }
        }
        Kind::Rpm => Action::Run { exe: "alien".into(), args: vec!["-i".into(), "--scripts".into(), absolute(&file)] },
        Kind::AppImage => Action::AppImage {
            file,
            target: v.target_dir(&p.name).unwrap_or_default(),
            name: p.name.clone(),
            version: p.version.clone(),
        },
    }
}

fn absolute(path: &Path) -> String {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()).display().to_string()
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
            Action::Unzip { zip, target, .. } | Action::AppImage { file: zip, target, .. } => {
                format!("dezarhivare {} → {}", zip.display(), target.display())
            }
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

    fn linux(kind: &str) -> Program {
        toml::from_str(&format!(
            "name = \"Furnica\"\nversion = \"3.0.5\"\ncategory = \"C\"\n[detect]\npackage = \"furnica\"\n[[variant]]\narch = \"x64\"\nkind = \"{kind}\"\nfile = \"f.{kind}\"\n"
        ))
        .unwrap()
    }

    #[test]
    fn deb_command() {
        let p = linux("deb");
        let file = std::path::absolute(Path::new("F").join("f.deb")).unwrap().display().to_string();
        assert_eq!(
            action_for(&p, &p.variants[0], Path::new("F")),
            Action::Run {
                exe: "env".into(),
                args: ["DEBIAN_FRONTEND=noninteractive", "apt-get", "install", "-y", &file].map(String::from).to_vec(),
            }
        );
    }

    #[test]
    fn rpm_command() {
        let p = linux("rpm");
        let file = std::path::absolute(Path::new("F").join("f.rpm")).unwrap().display().to_string();
        assert_eq!(
            action_for(&p, &p.variants[0], Path::new("F")),
            Action::Run { exe: "alien".into(), args: ["-i", "--scripts", &file].map(String::from).to_vec() }
        );
    }

    #[test]
    fn appimage_action() {
        let p = linux("appimage");
        assert_eq!(
            action_for(&p, &p.variants[0], Path::new("F")),
            Action::AppImage {
                file: Path::new("F").join("f.appimage"),
                target: "/opt/furnica".into(),
                name: "Furnica".into(),
                version: "3.0.5".into(),
            }
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
