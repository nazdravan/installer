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
}

impl SystemInfo {
    pub fn label(&self) -> String {
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
    }
}

/// Pe alte sisteme (dezvoltare pe macOS/Linux) se pretinde un Windows 10 obișnuit.
#[cfg(not(windows))]
pub fn detect() -> SystemInfo {
    SystemInfo { win: Some(WinVer::Win10), arch: Arch::X64, admin: false, computer: "test".into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_formats() {
        let sys = SystemInfo { win: Some(WinVer::Win8), arch: Arch::X86, admin: true, computer: "PC1".into() };
        assert_eq!(sys.label(), "Windows 8 · 32 biți · Administrator ✔");
        let sys = SystemInfo { win: None, arch: Arch::X64, admin: false, computer: "PC1".into() };
        assert_eq!(sys.label(), "Windows nesuportat · 64 biți · fără drepturi de administrator ❌");
    }
}
