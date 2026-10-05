#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn writes_one_user_message_line() {
        let line = message_line("Continue \"now\"\nplease");
        assert!(line.ends_with('\n'));
        let v: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(v["type"], "user");
        assert_eq!(v["message"]["role"], "user");
        assert_eq!(v["message"]["content"], "Continue \"now\"\nplease");
        assert_eq!(line.matches('\n').count(), 1, "a newline in the text must not split the line");
    }

    #[test]
    fn only_claude_code_inbox_sockets_are_used() {
        assert!(is_inbox_path(Path::new("/run/user/1000/cc-socks/64743.sock"), 1000));
        assert!(is_inbox_path(Path::new("/tmp/cc-socks-1000/64743.sock"), 1000));
        assert!(!is_inbox_path(Path::new("/run/user/1001/cc-socks/1.sock"), 1000));
        assert!(!is_inbox_path(Path::new("/run/user/1000/cc-socks/../x.sock"), 1000));
        assert!(!is_inbox_path(Path::new("/etc/passwd"), 1000));
    }
}
