//! Ce fel de Windows are calculatorul pe care rulează instalatorul.

use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum WinVer {
    /// Windows 8 și 8.1.
    #[serde(rename = "8")]
    Win8,
    /// Windows 10 și 11 (amândouă raportează 10.0).
    #[serde(rename = "10")]
    Win10,
}

impl WinVer {
    pub fn name(self) -> &'static str {
        match self {
            WinVer::Win8 => "Windows 8",
            WinVer::Win10 => "Windows 10",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    X86,
    X64,
}

impl Arch {
    pub fn bits(self) -> &'static str {
        match self {
            Arch::X86 => "32 biți",
            Arch::X64 => "64 biți",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SystemInfo {
    /// `None`: Windows 7 sau mai vechi, ori alt sistem de operare.
    pub win: Option<WinVer>,
    pub arch: Arch,
    pub admin: bool,
    pub computer: String,
    /// Numele distribuției (`PRETTY_NAME`) când instalatorul rulează pe Linux.
    pub linux: Option<String>,
}

impl SystemInfo {
    pub fn label(&self) -> String {
        if let Some(distro) = &self.linux {
            let root = if self.admin { "root ✔" } else { "fără root ❌" };
            return format!("{distro} · {} · {root}", self.arch.bits());
        }
        let win = self.win.map_or("Windows nesuportat", WinVer::name);
        let admin = if self.admin { "Administrator ✔" } else { "fără drepturi de administrator ❌" };
        format!("{win} · {} · {admin}", self.arch.bits())
    }
}

#[cfg(windows)]
pub fn detect() -> SystemInfo {
    use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
    use windows_sys::Win32::System::SystemInformation::{
        GetNativeSystemInfo, OSVERSIONINFOW, PROCESSOR_ARCHITECTURE_AMD64, PROCESSOR_ARCHITECTURE_ARM64,
        SYSTEM_INFO,
    };
    use windows_sys::Win32::UI::Shell::IsUserAnAdmin;

    // RtlGetVersion, nu GetVersionEx: acesta din urmă minte după manifestul programului.
    let mut ver: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
    ver.dwOSVersionInfoSize = size_of::<OSVERSIONINFOW>() as u32;
    unsafe { RtlGetVersion(&mut ver) };
    let win = match (ver.dwMajorVersion, ver.dwMinorVersion) {
        (6, 2 | 3) => Some(WinVer::Win8),
        (10, _) => Some(WinVer::Win10),
        _ => None,
    };

    // Arhitectura calculatorului, nu a procesului: instalatorul e mereu pe 32 de biți.
    let mut info: SYSTEM_INFO = unsafe { std::mem::zeroed() };
    unsafe { GetNativeSystemInfo(&mut info) };
    let arch = match unsafe { info.Anonymous.Anonymous.wProcessorArchitecture } {
        PROCESSOR_ARCHITECTURE_AMD64 | PROCESSOR_ARCHITECTURE_ARM64 => Arch::X64,
        _ => Arch::X86,
    };

    SystemInfo {
        win,
        arch,
        admin: unsafe { IsUserAnAdmin() } != 0,
        computer: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "calculator".into()),
        linux: None,
    }
}

#[cfg(target_os = "linux")]
pub fn detect() -> SystemInfo {
    let read = |path| std::fs::read_to_string(path).unwrap_or_default();
    let hostname = read("/etc/hostname").trim().to_string();
    SystemInfo {
        win: None,
        arch: if cfg!(target_arch = "x86_64") { Arch::X64 } else { Arch::X86 },
        admin: unsafe { libc::geteuid() } == 0,
        computer: if hostname.is_empty() { "calculator".into() } else { hostname },
        linux: Some(pretty_name(&read("/etc/os-release")).unwrap_or_else(|| "Linux".into())),
    }
}

/// Pe alte sisteme (dezvoltare pe macOS) se pretinde un Windows 10 obișnuit.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn detect() -> SystemInfo {
    SystemInfo { win: Some(WinVer::Win10), arch: Arch::X64, admin: false, computer: "test".into(), linux: None }
}

/// Valoarea lui `PRETTY_NAME` din `/etc/os-release`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn pretty_name(os_release: &str) -> Option<String> {
    let value = os_release.lines().find_map(|l| l.strip_prefix("PRETTY_NAME="))?;
    Some(value.trim().trim_matches('"').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_name_parses() {
        let text = "NAME=\"Linux Mint\"\nPRETTY_NAME=\"Linux Mint 21.3\"\nID=linuxmint\n";
        assert_eq!(pretty_name(text).as_deref(), Some("Linux Mint 21.3"));
        assert_eq!(pretty_name("PRETTY_NAME=antiX-23\n").as_deref(), Some("antiX-23"));
        assert_eq!(pretty_name("ID=x\n"), None);
    }

    #[test]
    fn label_linux() {
        let sys = SystemInfo { win: None, arch: Arch::X64, admin: true, computer: "PC".into(), linux: Some("Ubuntu 22.04".into()) };
        assert_eq!(sys.label(), "Ubuntu 22.04 · 64 biți · root ✔");
        let sys = SystemInfo { admin: false, ..sys };
        assert_eq!(sys.label(), "Ubuntu 22.04 · 64 biți · fără root ❌");
    }

    #[test]
    fn label_formats() {
        let sys = SystemInfo { win: Some(WinVer::Win8), arch: Arch::X86, admin: true, computer: "PC1".into(), linux: None };
        assert_eq!(sys.label(), "Windows 8 · 32 biți · Administrator ✔");
        let sys = SystemInfo { win: None, arch: Arch::X64, admin: false, computer: "PC1".into(), linux: None };
        assert_eq!(sys.label(), "Windows nesuportat · 64 biți · fără drepturi de administrator ❌");
    }
}
