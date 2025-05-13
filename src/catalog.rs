//! Programele din mapa `Windows` de pe stick: câte o submapă cu `program.toml`.

use std::path::{Path, PathBuf};

use crate::manifest::{self, Program};

pub struct Entry {
    pub folder: PathBuf,
    pub program: Result<Program, String>,
}

/// Citește toate submapele lui `root`, sortate după categorie și nume. Programele cu
/// descrierea greșită vin la sfârșit, ca să se vadă, dar să nu încurce lista.
pub fn scan(root: &Path) -> Vec<Entry> {
    let Ok(dir) = std::fs::read_dir(root) else { return Vec::new() };
    let mut entries: Vec<Entry> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .map(|folder| {
            let program = match std::fs::read_to_string(folder.join("program.toml")) {
                Ok(text) => manifest::parse(&text, &folder),
                Err(_) => Err("lipsește program.toml".into()),
            };
            Entry { folder, program }
        })
        .collect();
    entries.sort_by(|a, b| match (&a.program, &b.program) {
        (Ok(pa), Ok(pb)) => (&pa.category, &pa.name).cmp(&(&pb.category, &pb.name)),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => a.folder.cmp(&b.folder),
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(dir: &Path, folder: &str, category: &str) {
        let d = dir.join(folder);
        std::fs::create_dir(&d).unwrap();
        std::fs::write(d.join("a.zip"), b"").unwrap();
        let text = format!(
            "name = \"{folder}\"\nversion = \"1\"\ncategory = \"{category}\"\n[[variant]]\nwindows = [\"10\"]\narch = \"x86\"\nkind = \"zip\"\nfile = \"a.zip\"\ntarget = 'C:\\x'\nshortcut = \"x.exe\"\n"
        );
        std::fs::write(d.join("program.toml"), text).unwrap();
    }

    #[test]
    fn scan_sorts_and_reports_errors() {
        let dir = tempfile::tempdir().unwrap();
        program(dir.path(), "A", "Educațional");
        program(dir.path(), "B", "Birou");
        std::fs::create_dir(dir.path().join("Z")).unwrap();
        std::fs::write(dir.path().join("readme.txt"), b"").unwrap();

        let entries = scan(dir.path());
        let names: Vec<_> = entries.iter().map(|e| e.folder.file_name().unwrap().to_str().unwrap()).collect();
        assert_eq!(names, ["B", "A", "Z"]);
        assert_eq!(entries[2].program.as_ref().unwrap_err(), "lipsește program.toml");
    }

    #[test]
    fn scan_missing_root_is_empty() {
        assert!(scan(Path::new("/nu/exista/deloc")).is_empty());
    }
}
