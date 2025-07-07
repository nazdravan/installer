//! Rularea efectivă a instalării.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::command::Action;
use crate::detect::{self, MARKER};
use crate::manifest::Variant;

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Ok,
    OkReboot,
    Failed(String),
}

pub fn classify(code: i32, v: &Variant) -> Outcome {
    if v.success_codes.contains(&code) {
        Outcome::Ok
    } else if v.reboot_codes.contains(&code) {
        Outcome::OkReboot
    } else {
        Outcome::Failed(format!("cod de ieșire {code}"))
    }
}

/// Rulează programul și așteaptă cel mult `timeout`. Un installer care, în ciuda cheilor
/// silențioase, deschide o fereastră și așteaptă un click e oprit, ca să nu blocheze coada.
/// Întoarce codul de ieșire și ultima linie scrisă pe stderr (de ex. mesajul lui apt).
pub fn run_process(exe: &Path, args: &[String], timeout: Duration) -> Result<(i32, String), String> {
    let mut command = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
        for arg in args {
            command.raw_arg(windows_arg(arg));
        }
    }
    #[cfg(not(windows))]
    command.args(args);
    wait_with_timeout(command, exe, timeout)
}

/// Pornește comanda și o așteaptă împreună cu procesele pornite de ea (un setup.exe care pornește
/// setup.tmp, apt-get care pornește dpkg): la depășirea timpului se opresc toate, altfel ar ține
/// ocupat Windows Installer sau lacătul lui dpkg pentru programele următoare.
fn wait_with_timeout(mut command: Command, exe: &Path, timeout: Duration) -> Result<(i32, String), String> {
    use std::io::BufRead;
    use std::process::Stdio;
    use std::sync::{Arc, Mutex};

    // Fără stdin: dpkg sau un installer nu are cum să aștepte un răspuns din terminal.
    command.stdin(Stdio::null()).stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound if exe == Path::new("alien") => "lipsește alien (sudo apt install alien)".to_string(),
        std::io::ErrorKind::NotFound => format!("lipsește {}", exe.display()),
        _ => format!("nu pot porni {}: {e}", exe.display()),
    })?;
    #[cfg(windows)]
    let job = job::Job::for_child(&child);

    // Firul nu se așteaptă la sfârșit: un proces rămas pornit poate ține pipe-ul deschis.
    let last_line = Arc::new(Mutex::new(String::new()));
    if let Some(stderr) = child.stderr.take() {
        let last_line = Arc::clone(&last_line);
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    *last_line.lock().unwrap() = line.trim().to_string();
                }
            }
        });
    }
    let stderr = || {
        std::thread::sleep(Duration::from_millis(100));
        last_line.lock().unwrap().clone()
    };

    let start = Instant::now();
    let mut code = None;
    loop {
        if code.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => code = Some(status.code().unwrap_or(-1)),
                Ok(None) => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        // Pe Windows se așteaptă și procesele pornite de installer (un bootstrapper iese imediat).
        #[cfg(windows)]
        let others_running = job.as_ref().is_some_and(|j| j.active() > 0);
        #[cfg(not(windows))]
        let others_running = false;
        if let (Some(code), false) = (code, others_running) {
            return Ok((code, stderr()));
        }
        if start.elapsed() >= timeout {
            #[cfg(windows)]
            if let Some(job) = &job {
                job.kill();
            }
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            // Installerul a terminat, doar ce a pornit după el (de ex. programul) a rămas deschis.
            if let Some(code) = code {
                return Ok((code, stderr()));
            }
            return Err(format!("a depășit timpul ({} min)", timeout.as_secs().div_ceil(60)));
        }
        std::thread::sleep(Duration::from_millis(500).min(timeout));
    }
}

#[cfg(windows)]
mod job {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        JobObjectBasicAccountingInformation, QueryInformationJobObject, TerminateJobObject,
    };

    /// Un Job Object cu installerul și toate procesele pornite de el.
    pub struct Job(HANDLE);

    impl Job {
        pub fn for_child(child: &std::process::Child) -> Option<Job> {
            use std::os::windows::io::AsRawHandle;
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                if AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) == 0 {
                    CloseHandle(job);
                    return None;
                }
                Some(Job(job))
            }
        }

        pub fn active(&self) -> u32 {
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
            let ok = unsafe {
                QueryInformationJobObject(
                    self.0,
                    JobObjectBasicAccountingInformation,
                    &mut info as *mut _ as *mut _,
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 { 0 } else { info.ActiveProcesses }
        }

        pub fn kill(&self) {
            unsafe { TerminateJobObject(self.0, 1) };
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Pune conținutul arhivei în `target`, înlocuind ce era acolo. Dacă mapa veche nu poate fi
/// înlocuită (programul e deschis), ea rămâne neatinsă.
pub fn unzip_replace(zip: &Path, target: &Path) -> Result<(), String> {
    let staging = sibling(target, "nou");
    let _ = std::fs::remove_dir_all(&staging);
    if let Err(e) = extract(zip, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    replace_dir(&staging, target)
}

/// Pune `staging` în locul lui `target`. Dacă `target` nu poate fi mutat (program deschis),
/// rămâne neatins, iar `staging` se șterge.
fn replace_dir(staging: &Path, target: &Path) -> Result<(), String> {
    let old = sibling(target, "vechi");
    let _ = std::fs::remove_dir_all(&old);
    if target.exists() && std::fs::rename(target, &old).is_err() {
        let _ = std::fs::remove_dir_all(staging);
        return Err(format!("nu pot înlocui {}: programul este deschis?", target.display()));
    }
    if let Err(e) = std::fs::rename(staging, target) {
        let _ = std::fs::rename(&old, target);
        let _ = std::fs::remove_dir_all(staging);
        return Err(format!("nu pot crea {}: {e}", target.display()));
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(())
}

/// `C:\Program Files\Furnica` → `C:\Program Files\Furnica.<suffix>`
fn sibling(target: &Path, suffix: &str) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{suffix}"));
    target.with_file_name(name)
}

/// Dezarhivează în `dest`. Arhivele portabile au de obicei un singur dosar de sus
/// (`furnica-3.0.4-windows7-x86-portable/`), care se elimină din căi.
fn extract(zip: &Path, dest: &Path) -> Result<(), String> {
    let err = |e: &dyn std::fmt::Display| format!("{}: {e}", zip.display());
    let file = std::fs::File::open(zip).map_err(|e| err(&e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| err(&e))?;
    // Intrările care ar ieși din `dest` (`..`, căi absolute) se sar.
    let names: Vec<Option<(PathBuf, bool)>> = (0..archive.len())
        .map(|i| archive.by_index(i).ok().and_then(|f| Some((f.enclosed_name()?, f.is_dir()))))
        .collect();
    let top = single_top_dir(&names.iter().flatten().cloned().collect::<Vec<_>>());
    std::fs::create_dir_all(dest).map_err(|e| format!("nu pot crea {}: {e}", dest.display()))?;
    for (i, name) in names.iter().enumerate() {
        let Some((name, _)) = name else { continue };
        let rel = match &top {
            Some(top) => name.strip_prefix(top).unwrap_or(name),
            None => name,
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dest.join(rel);
        let mut entry = archive.by_index(i).map_err(|e| err(&e))?;
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| err(&e))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| err(&e))?;
        }
        let mut f = std::fs::File::create(&out).map_err(|e| err(&e))?;
        std::io::copy(&mut entry, &mut f).map_err(|e| err(&e))?;
    }
    Ok(())
}

/// Dosarul comun tuturor intrărilor, dacă există; un fișier la rădăcină înseamnă că nu există.
fn single_top_dir(entries: &[(PathBuf, bool)]) -> Option<PathBuf> {
    let top = PathBuf::from(entries.first()?.0.components().next()?.as_os_str());
    let inside = |(name, is_dir): &(PathBuf, bool)| name.starts_with(&top) && (*is_dir || name != &top);
    entries.iter().all(inside).then_some(top)
}

/// Intrarea din meniul de aplicații pentru un AppImage dezarhivat în `target`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn desktop_entry(name: &str, target: &Path) -> String {
    let t = target.display();
    format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec=\"{t}/AppRun\"\nIcon={t}/.DirIcon\nCategories=Education;\nTerminal=false\n"
    )
}

#[cfg(unix)]
fn execute_appimage(file: &Path, target: &Path, name: &str, version: &str, timeout: Duration) -> Outcome {
    if detect::is_foreign(target) {
        return Outcome::Failed(foreign_message(target));
    }
    if let Err(e) = install_appimage(file, target, timeout) {
        return Outcome::Failed(e);
    }
    if let Err(e) = std::fs::write(target.join(MARKER), version) {
        return Outcome::Failed(format!("nu pot scrie versiunea: {e}"));
    }
    #[cfg(target_os = "linux")]
    {
        let slug = name.to_lowercase().replace(' ', "-");
        let entry = format!("/usr/share/applications/instalator-{slug}.desktop");
        if let Err(e) = std::fs::write(&entry, desktop_entry(name, target)) {
            eprintln!("nu pot crea {entry}: {e}");
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = name;
    Outcome::Ok
}

#[cfg(not(unix))]
fn execute_appimage(file: &Path, _target: &Path, _name: &str, _version: &str, _timeout: Duration) -> Outcome {
    Outcome::Failed(format!("AppImage merge doar pe Linux: {}", file.display()))
}

/// AppImage-ul se dezarhivează (`--appimage-extract`) în loc să fie rulat: așa nu are nevoie
/// de FUSE, care lipsește pe Ubuntu 22.04 și mai nou.
#[cfg(unix)]
fn install_appimage(file: &Path, target: &Path, timeout: Duration) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let work = std::env::temp_dir().join(format!("instalator-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    let result = (|| {
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        let copy = work.join("program.AppImage");
        std::fs::copy(file, &copy).map_err(|e| format!("nu pot copia {}: {e}", file.display()))?;
        std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        let mut command = Command::new(&copy);
        command.arg("--appimage-extract").current_dir(&work).stdout(std::process::Stdio::null());
        match wait_with_timeout(command, &copy, timeout)? {
            (0, _) => {}
            (code, _) => return Err(format!("dezarhivarea AppImage a eșuat (cod {code})")),
        }
        let staging = sibling(target, "nou");
        let _ = std::fs::remove_dir_all(&staging);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("nu pot crea {}: {e}", parent.display()))?;
        }
        // squashfs-root e în /tmp, posibil pe altă partiție decât target: copiere, nu redenumire.
        copy_dir(&work.join("squashfs-root"), &staging)?;
        replace_dir(&staging, target)
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

#[cfg(unix)]
fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    let err = |e: std::io::Error| format!("{}: {e}", from.display());
    std::fs::create_dir_all(to).map_err(err)?;
    for entry in std::fs::read_dir(from).map_err(err)? {
        let entry = entry.map_err(err)?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type().map_err(err)?;
        if kind.is_symlink() {
            let link = std::fs::read_link(&src).map_err(err)?;
            std::os::unix::fs::symlink(link, &dst).map_err(err)?;
        } else if kind.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(err)?;
        }
    }
    Ok(())
}

fn foreign_message(target: &Path) -> String {
    format!("în {} există deja o altă instalare (de exemplu din .msi); dezinstalați-o întâi", target.display())
}

/// Cum ajunge un argument în linia de comandă de pe Windows. Argumentele scrise de profesor se
/// dau exact cum le-ar scrie în cmd (msiexec, NSIS și Inno își citesc singure linia, nu după
/// regulile MSVCRT); doar unul fără ghilimele care conține spații, ca o cale, primește ghilimele.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn windows_arg(arg: &str) -> String {
    let plain = !arg.contains('"') && !arg.starts_with("/D=");
    if plain && (arg.is_empty() || arg.contains(char::is_whitespace)) {
        // Un „\” de la sfârșit ar scăpa ghilimelele de închidere.
        let tail = arg.len() - arg.trim_end_matches('\\').len();
        format!("\"{arg}{}\"", "\\".repeat(tail))
    } else {
        arg.to_string()
    }
}

pub fn execute(action: &Action, v: &Variant) -> Outcome {
    match action {
        Action::Run { exe, args } => match run_process(exe, args, Duration::from_secs(v.timeout_min * 60)) {
            Ok((code, stderr)) => match classify(code, v) {
                Outcome::Failed(why) if !stderr.is_empty() => Outcome::Failed(format!("{why}: {stderr}")),
                outcome => outcome,
            },
            Err(e) => Outcome::Failed(e),
        },
        Action::Unzip { zip, target, shortcut_exe, name, desktop, version } => {
            if detect::is_foreign(target) {
                return Outcome::Failed(foreign_message(target));
            }
            if let Err(e) = unzip_replace(zip, target) {
                return Outcome::Failed(e);
            }
            if let Err(e) = std::fs::write(target.join(MARKER), version) {
                return Outcome::Failed(format!("nu pot scrie versiunea: {e}"));
            }
            #[cfg(windows)]
            shortcuts(name, &target.join(shortcut_exe), *desktop);
            #[cfg(not(windows))]
            let _ = (name, shortcut_exe, desktop);
            Outcome::Ok
        }
        Action::AppImage { file, target, name, version } => {
            execute_appimage(file, target, name, version, Duration::from_secs(v.timeout_min * 60))
        }
    }
}

/// Scurtăturile pentru toți utilizatorii: în meniul Start și, dacă se cere, pe desktop.
/// O scurtătură care nu se poate crea nu face instalarea să eșueze.
#[cfg(windows)]
fn shortcuts(name: &str, exe: &Path, desktop: bool) {
    let mut places = Vec::new();
    if let Ok(dir) = std::env::var("ProgramData") {
        places.push(PathBuf::from(dir).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if desktop && let Ok(dir) = std::env::var("PUBLIC") {
        places.push(PathBuf::from(dir).join("Desktop"));
    }
    for dir in places {
        let lnk = dir.join(format!("{name}.lnk"));
        let result = mslnk::ShellLink::new(exe).and_then(|mut link| {
            link.set_working_dir(exe.parent().map(|d| d.display().to_string()));
            link.create_lnk(&lnk)
        });
        if let Err(e) = result {
            eprintln!("nu pot crea scurtătura {}: {e}", lnk.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn variant(success: &str) -> Variant {
        let p: crate::manifest::Program = toml::from_str(&format!(
            "name = \"P\"\nversion = \"1\"\ncategory = \"C\"\n[[variant]]\nwindows = [\"10\"]\narch = \"x86\"\nkind = \"exe\"\nfile = \"f\"\n{success}"
        ))
        .unwrap();
        p.variants[0].clone()
    }

    fn make_zip(path: &Path, files: &[&str]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for f in files {
            zip.start_file(*f, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(b"x").unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn classify_codes() {
        let v = variant("");
        assert_eq!(classify(0, &v), Outcome::Ok);
        assert_eq!(classify(3010, &v), Outcome::OkReboot);
        assert_eq!(classify(1603, &v), Outcome::Failed("cod de ieșire 1603".into()));
        assert_eq!(classify(1, &variant("success_codes = [0, 1]")), Outcome::Ok);
    }

    #[cfg(unix)]
    #[test]
    fn run_process_exit_code() {
        let args = ["-c".to_string(), "exit 7".to_string()];
        assert_eq!(run_process(Path::new("/bin/sh"), &args, Duration::from_secs(10)), Ok((7, String::new())));
    }

    #[cfg(unix)]
    #[test]
    fn run_process_kills_on_timeout() {
        let start = Instant::now();
        let err = run_process(Path::new("/bin/sleep"), &["30".into()], Duration::from_secs(1)).unwrap_err();
        assert!(err.contains("timpul"), "{err}");
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn unzip_strips_single_top_dir() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("a.zip");
        make_zip(&zip, &["Furnica-3.0.4/Furnica.exe", "Furnica-3.0.4/soft-gl/opengl32.dll"]);
        let target = dir.path().join("Furnica");
        unzip_replace(&zip, &target).unwrap();
        assert!(target.join("Furnica.exe").is_file());
        assert!(target.join("soft-gl/opengl32.dll").is_file());
    }

    #[test]
    fn unzip_replaces_old_files() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("a.zip");
        make_zip(&zip, &["Furnica.exe", "LICENTE.txt"]);
        let target = dir.path().join("Furnica");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("vechi.txt"), b"").unwrap();
        unzip_replace(&zip, &target).unwrap();
        assert!(target.join("Furnica.exe").is_file());
        assert!(!target.join("vechi.txt").exists());
        assert!(!dir.path().join("Furnica.nou").exists());
        assert!(!dir.path().join("Furnica.vechi").exists());
    }

    #[cfg(unix)]
    #[test]
    fn unzip_replace_keeps_old_on_failure() {
        use std::os::unix::fs::PermissionsExt;
        // root ignoră drepturile de acces, deci blocajul nu poate fi simulat așa.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("a.zip");
        make_zip(&zip, &["Furnica.exe"]);
        let parent = dir.path().join("pf");
        let target = parent.join("Furnica");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("vechi.txt"), b"").unwrap();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o555)).unwrap();
        let result = unzip_replace(&zip, &target);
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(result.is_err());
        assert!(target.join("vechi.txt").is_file());
    }

    #[test]
    fn unzip_rejects_escaping_paths() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("a.zip");
        make_zip(&zip, &["../afara.txt", "Furnica.exe"]);
        let target = dir.path().join("t").join("Furnica");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        let _ = unzip_replace(&zip, &target);
        assert!(!dir.path().join("t").join("afara.txt").exists());
    }

    #[test]
    fn windows_args_verbatim_unless_plain_with_spaces() {
        assert_eq!(windows_arg(r"F:\Sumatra PDF\setup.exe"), r#""F:\Sumatra PDF\setup.exe""#);
        assert_eq!(windows_arg("/qn"), "/qn");
        assert_eq!(windows_arg(r#"DESKTOPSHORTCUT="""#), r#"DESKTOPSHORTCUT="""#);
        assert_eq!(windows_arg(r#"INSTALLDIR="C:\Program Files\X""#), r#"INSTALLDIR="C:\Program Files\X""#);
        // NSIS recunoaște /D= doar fără ghilimele.
        assert_eq!(windows_arg(r"/D=C:\Program Files\X"), r"/D=C:\Program Files\X");
    }

    #[cfg(unix)]
    #[test]
    fn run_process_kills_grandchildren() {
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join("pid");
        let script = format!("sleep 30 & echo $! > '{}'; wait", pidfile.display());
        let err = run_process(Path::new("/bin/sh"), &["-c".into(), script], Duration::from_secs(1)).unwrap_err();
        assert!(err.contains("timpul"));
        let pid: i32 = std::fs::read_to_string(&pidfile).unwrap().trim().parse().unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "procesul-nepot {pid} trăiește încă");
    }

    #[cfg(unix)]
    #[test]
    fn failure_shows_last_stderr_line() {
        let action = Action::Run {
            exe: "/bin/sh".into(),
            args: vec!["-c".into(), "echo prima >&2; echo 'E: Could not get lock' >&2; exit 100".into()],
        };
        assert_eq!(execute(&action, &variant("")), Outcome::Failed("cod de ieșire 100: E: Could not get lock".into()));
    }

    #[test]
    fn zip_refuses_foreign_install() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("a.zip");
        make_zip(&zip, &["Furnica.exe"]);
        let target = dir.path().join("Furnica");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("din-msi.txt"), b"").unwrap();
        let action = Action::Unzip {
            zip,
            target: target.clone(),
            shortcut_exe: "Furnica.exe".into(),
            name: "Furnica".into(),
            desktop: false,
            version: "3.0.4".into(),
        };
        match execute(&action, &variant("")) {
            Outcome::Failed(why) => assert!(why.contains("altă instalare"), "{why}"),
            other => panic!("{other:?}"),
        }
        assert!(target.join("din-msi.txt").is_file());
    }

    #[test]
    fn missing_alien_hint() {
        let action = Action::Run { exe: "alien".into(), args: vec!["-i".into()] };
        match execute(&action, &variant("")) {
            Outcome::Failed(why) => assert!(why.contains("sudo apt install alien"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn appimage_extracts_into_target() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        // Un „AppImage” fals: la --appimage-extract creează squashfs-root, ca unul adevărat.
        let file = dir.path().join("p.AppImage");
        std::fs::write(&file, "#!/bin/sh\n[ \"$1\" = --appimage-extract ] && mkdir -p squashfs-root && echo x > squashfs-root/AppRun\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let target = dir.path().join("opt").join("p");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        let action = Action::AppImage { file, target: target.clone(), name: "P".into(), version: "2.0".into() };
        assert_eq!(execute(&action, &variant("")), Outcome::Ok);
        assert!(target.join("AppRun").is_file());
        assert_eq!(std::fs::read_to_string(target.join(MARKER)).unwrap(), "2.0");
    }

    /// Instalare reală cu apt-get și verificare cu dpkg. Cere root și Debian/Ubuntu, deci rulează
    /// doar explicit, de ex. în container: `cargo test -- --ignored real_deb_install`.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn real_deb_install() {
        use crate::detect::{Dpkg, Registry};
        let dir = tempfile::tempdir().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(pkg.join("DEBIAN")).unwrap();
        std::fs::create_dir_all(pkg.join("usr/share/doc/instalator-test")).unwrap();
        std::fs::write(pkg.join("usr/share/doc/instalator-test/x"), "x").unwrap();
        std::fs::write(
            pkg.join("DEBIAN/control"),
            "Package: instalator-test\nVersion: 1:2.5-1\nArchitecture: all\nMaintainer: t <t@t>\nDescription: test\n",
        )
        .unwrap();
        let deb = dir.path().join("instalator-test.deb");
        assert!(Command::new("dpkg-deb").args(["--build"]).arg(&pkg).arg(&deb).status().unwrap().success());

        let p: crate::manifest::Program = toml::from_str(
            "name = \"T\"\nversion = \"2.5\"\ncategory = \"C\"\n[detect]\npackage = \"instalator-test\"\n[[variant]]\narch = \"x64\"\nkind = \"deb\"\nfile = \"instalator-test.deb\"\n",
        )
        .unwrap();
        let action = crate::command::action_for(&p, &p.variants[0], dir.path());
        assert_eq!(execute(&action, &p.variants[0]), Outcome::Ok);
        assert_eq!(Dpkg.package_version("instalator-test").as_deref(), Some("1:2.5-1"));
        let installed = crate::detect::installed_version(&p, &p.variants[0], &Dpkg);
        assert_eq!(installed.as_deref(), Some("2.5-1"));
        let _ = Command::new("dpkg").args(["--purge", "instalator-test"]).status();
        assert_eq!(Dpkg.package_version("instalator-test"), None);
    }

    #[test]
    fn desktop_entry_format() {
        let text = desktop_entry("Furnica", Path::new("/opt/furnica"));
        assert!(text.starts_with("[Desktop Entry]\n"));
        for line in ["Name=Furnica", "Exec=\"/opt/furnica/AppRun\"", "Icon=/opt/furnica/.DirIcon", "Type=Application", "Categories=Education;"] {
            assert!(text.lines().any(|l| l == line), "{line}\n{text}");
        }
    }

    #[test]
    fn execute_zip_writes_marker() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("a.zip");
        make_zip(&zip, &["Furnica.exe"]);
        let target = dir.path().join("Furnica");
        let action = Action::Unzip {
            zip,
            target: target.clone(),
            shortcut_exe: "Furnica.exe".into(),
            name: "Furnica".into(),
            desktop: false,
            version: "3.0.4".into(),
        };
        let v = variant("");
        assert_eq!(execute(&action, &v), Outcome::Ok);
        assert_eq!(std::fs::read_to_string(target.join(MARKER)).unwrap(), "3.0.4");
    }
}
