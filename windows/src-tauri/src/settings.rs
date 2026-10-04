// Preferences, stored as plain JSON in %APPDATA%\Coucou\settings.json on
// Windows and ~/.config/coucou/settings.json on Linux. No secret ever lands
// here — API keys live in the Windows Credential Manager or the Secret Service.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub sound_enabled: bool,
    pub sound_volume: f64,
    pub auto_close_interval: f64,
    pub absence_interval: f64,
    pub active_integrations: Vec<String>,
    /// "primary" = the main display, "cursor" = whichever display the mouse is on.
    pub screen: String,
    pub autostart: bool,
    pub hooks_installed: bool,
    /// Claude model used by the chat. Changeable in the settings window.
    /// Defaulted explicitly so a settings.json written by an older build still loads.
    #[serde(default = "default_model")]
    pub model: String,
    /// Folders whose git repos are scanned for stale worktrees (`~/` allowed).
    #[serde(default = "default_worktree_roots")]
    pub worktree_roots: Vec<String>,
    /// A clean worktree untouched this many days counts as stale.
    #[serde(default = "default_worktree_idle_days")]
    pub worktree_idle_days: u64,
    /// Once a day, remove clean worktrees untouched for `worktree_auto_days`.
    #[serde(default)]
    pub worktree_auto_clean: bool,
    #[serde(default = "default_worktree_auto_days")]
    pub worktree_auto_days: u64,
    /// GitHub repos (`owner/name`) whose Actions runs the Pipelines view shows.
    #[serde(default)]
    pub pipeline_repos: Vec<String>,
}

fn default_worktree_auto_days() -> u64 {
    7
}

fn default_worktree_roots() -> Vec<String> {
    vec!["~/IdeaProjects".into()]
}

fn default_worktree_idle_days() -> u64 {
    14
}

fn default_model() -> String {
    crate::claude::DEFAULT_MODEL.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            sound_volume: 0.12,
            auto_close_interval: 15.0,
            absence_interval: 180.0,
            active_integrations: vec![
                "integration_resend".into(),
                "integration_n8n".into(),
                "integration_vercel".into(),
                "integration_github".into(),
            ],
            screen: "primary".into(),
            autostart: false,
            hooks_installed: false,
            model: default_model(),
            worktree_roots: default_worktree_roots(),
            worktree_idle_days: default_worktree_idle_days(),
            worktree_auto_clean: false,
            worktree_auto_days: default_worktree_auto_days(),
            pipeline_repos: Vec::new(),
        }
    }
}

/// %APPDATA%\Coucou, or ~/.config/coucou.
pub fn config_dir() -> PathBuf {
    crate::platform::config_dir()
}

/// %LOCALAPPDATA%\Coucou, or ~/.local/share/coucou — where the relay and the log live.
pub fn local_dir() -> PathBuf {
    crate::platform::local_dir()
}

pub fn hook_exe_path() -> PathBuf {
    local_dir().join("bin").join(crate::platform::HOOK_EXE)
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn load() -> Settings {
    match std::fs::read(settings_path()) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_vec_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(settings_path(), json)
}
