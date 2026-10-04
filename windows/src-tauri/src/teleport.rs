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
