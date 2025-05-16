//! Jurnalul de pe stick: ce s-a întâmplat pe fiecare calculator.

use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct Journal {
    file: Option<File>,
}

impl Journal {
    /// Creează `dir/<calculator>-<dată-oră>.txt`. Dacă stick-ul e protejat la scriere,
    /// jurnalul pur și simplu lipsește: instalarea nu trebuie să se oprească din cauza lui.
    pub fn open(dir: &Path, computer: &str) -> Journal {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let file = std::fs::create_dir_all(dir)
            .and_then(|_| File::create(dir.join(format!("{computer}-{stamp}.txt"))))
            // Notepad-ul vechi recunoaște UTF-8 doar cu BOM.
            .and_then(|mut f| f.write_all("\u{feff}".as_bytes()).map(|_| f))
            .ok();
        Journal { file }
    }

    pub fn line(&mut self, text: &str) {
        if let Some(f) = &mut self.file {
            let _ = write!(f, "{} {text}\r\n", chrono::Local::now().format("%H:%M:%S"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_writes_lines() {
        let dir = tempfile::tempdir().unwrap();
        let jdir = dir.path().join("jurnal");
        let mut j = Journal::open(&jdir, "PC1");
        j.line("Furnica: ✓");
        drop(j);
        let files: Vec<_> = std::fs::read_dir(&jdir).unwrap().flatten().collect();
        assert_eq!(files.len(), 1);
        let name = files[0].file_name().into_string().unwrap();
        assert!(name.starts_with("PC1-") && name.ends_with(".txt"), "{name}");
        let text = std::fs::read_to_string(files[0].path()).unwrap();
        assert!(text.starts_with('\u{feff}'));
        assert!(text.contains("Furnica: ✓\r\n"));
    }

    #[cfg(unix)]
    #[test]
    fn journal_open_on_readonly_is_silent() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        let mut j = Journal::open(&dir.path().join("jurnal"), "PC1");
        j.line("x");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}
