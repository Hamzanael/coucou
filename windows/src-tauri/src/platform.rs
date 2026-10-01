// The few things that differ between Windows and Linux outside the island
// window itself: where files live, what the relay is called, the local time,
// and how to hand a URL or a folder to the desktop.

use std::path::PathBuf;
use std::process::Command;

/// File name of the Claude Code relay.
#[cfg(windows)]
pub const HOOK_EXE: &str = "coucou-hook.exe";
#[cfg(not(windows))]
pub const HOOK_EXE: &str = "coucou-hook";

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The user's home: %USERPROFILE% on Windows, $HOME elsewhere.
pub fn home() -> PathBuf {
    #[cfg(windows)]
    let home = env_dir("USERPROFILE");
    #[cfg(not(windows))]
    let home = env_dir("HOME");
    home.unwrap_or_else(|| PathBuf::from("."))
}

/// Preferences: %APPDATA%\Coucou, or $XDG_CONFIG_HOME/coucou (~/.config/coucou).
pub fn config_dir() -> PathBuf {
    #[cfg(windows)]
    return env_dir("APPDATA").unwrap_or_else(|| PathBuf::from(".")).join("Coucou");
    #[cfg(not(windows))]
    return env_dir("XDG_CONFIG_HOME")
        .unwrap_or_else(|| home().join(".config"))
        .join("coucou");
}

/// The relay, the log and the inbox: %LOCALAPPDATA%\Coucou, or
/// $XDG_DATA_HOME/coucou (~/.local/share/coucou).
pub fn local_dir() -> PathBuf {
    #[cfg(windows)]
    return env_dir("LOCALAPPDATA").unwrap_or_else(|| PathBuf::from(".")).join("Coucou");
    #[cfg(not(windows))]
    return env_dir("XDG_DATA_HOME")
        .unwrap_or_else(|| home().join(".local").join("share"))
        .join("coucou");
}

/// Local wall-clock time as (year, month, day, hour, minute, second).
#[cfg(windows)]
pub fn local_time() -> (u32, u32, u32, u32, u32, u32) {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    (
        t.wYear as u32,
        t.wMonth as u32,
        t.wDay as u32,
        t.wHour as u32,
        t.wMinute as u32,
        t.wSecond as u32,
    )
}

#[cfg(not(windows))]
pub fn local_time() -> (u32, u32, u32, u32, u32, u32) {
    // SAFETY: localtime_r only writes into the tm we own.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return (1970, 1, 1, 0, 0, 0);
        }
        (
            (tm.tm_year + 1900) as u32,
            (tm.tm_mon + 1) as u32,
            tm.tm_mday as u32,
            tm.tm_hour as u32,
            tm.tm_min as u32,
            tm.tm_sec as u32,
        )
    }
}

/// Keeps spawned helpers from flashing a console window on Windows.
pub fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Opens an http(s) URL in the default browser. The caller has checked the scheme.
pub fn open_url(url: &str) {
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("rundll32.exe");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    let _ = quiet(&mut cmd).spawn();
}

/// Shows a folder in the file manager: Explorer, or whatever xdg-open picks.
pub fn open_folder(path: &str) {
    #[cfg(windows)]
    let _ = Command::new("explorer").arg(path).spawn();
    #[cfg(not(windows))]
    let _ = Command::new("xdg-open").arg(path).spawn();
}
