// Pulls a Claude Code cloud session (claude.ai/code) into a local terminal:
// Ghostty, in the chosen repo, running `claude --teleport` — with the session
// from a copied claude.ai/code link when there is one, else Claude's own
// picker of your cloud sessions.

use std::path::Path;
use std::process::Command;

/// The `session_…` id in a claude.ai/code link or a bare id.
pub fn session_ref(text: &str) -> Option<String> {
    let start = text.find("session_")?;
    let id: String = text[start..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    (id.len() > "session_".len()).then_some(id)
}

pub fn command(dir: &str, session: Option<&str>, claude: &str) -> Vec<String> {
    let mut args = vec![format!("--working-directory={dir}"), "-e".into(), claude.into(), "--teleport".into()];
    if let Some(s) = session {
        args.push(s.into());
    }
    args
}

/// Opens Ghostty in `dir`. `clipboard` is whatever the page could read; only a
/// session id is ever taken from it.
pub fn launch(dir: &str, clipboard: Option<&str>) -> Result<String, String> {
    if !Path::new(dir).is_dir() {
        return Err(format!("{dir} is not a folder"));
    }
    let ghostty = crate::find_on_path("ghostty").ok_or("Ghostty is not installed")?;
    let session = clipboard.and_then(session_ref);
    let claude = crate::platform::home().join(".local/bin/claude");
    let claude = if claude.is_file() { claude.to_string_lossy().to_string() } else { "claude".to_string() };
    Command::new(ghostty)
        .args(command(dir, session.as_deref(), &claude))
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(match session {
        Some(s) => format!("Teleporting {s}"),
        None => "Pick a cloud session in the terminal".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_cloud_session_in_a_link_or_id() {
        assert_eq!(session_ref("https://claude.ai/code/session_01AbC9xyz"), Some("session_01AbC9xyz".into()));
        assert_eq!(session_ref("  session_01DS6Sp4rXbN1XiUYPNj6oYo\n"), Some("session_01DS6Sp4rXbN1XiUYPNj6oYo".into()));
        assert_eq!(session_ref("https://claude.ai/code/session_01Ab?tab=1"), Some("session_01Ab".into()));
        assert_eq!(session_ref("hello world"), None);
        assert_eq!(session_ref(""), None);
    }

    #[test]
    fn builds_the_ghostty_command() {
        let c = command("/home/u/IdeaProjects/analytickBE", Some("session_01Ab"), "/home/u/.local/bin/claude");
        assert_eq!(c, vec![
            "--working-directory=/home/u/IdeaProjects/analytickBE".to_string(),
            "-e".into(), "/home/u/.local/bin/claude".into(), "--teleport".into(), "session_01Ab".into(),
        ]);
        let picker = command("/p", None, "claude");
        assert_eq!(picker.last().map(String::as_str), Some("--teleport"));
    }
}
