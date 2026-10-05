// Sends a message into a running Claude Code session through its inbox socket —
// Claude Code's documented cross-session messaging. The session sees it as a
// message from another program, not as you typing (it can't approve anything);
// an idle session starts a turn with it.

use std::path::Path;

pub fn message_line(text: &str) -> String {
    serde_json::json!({ "type": "user", "message": { "role": "user", "content": text } }).to_string() + "\n"
}

/// Only Claude Code's own per-user socket folders, never a path that climbs out.
pub fn is_inbox_path(path: &Path, uid: u32) -> bool {
    let s = path.to_string_lossy();
    if s.contains("..") || !s.ends_with(".sock") {
        return false;
    }
    s.starts_with(&format!("/run/user/{uid}/cc-socks/")) || s.starts_with(&format!("/tmp/cc-socks-{uid}/"))
}

#[cfg(unix)]
pub fn send(socket: &str, text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    // SAFETY: getuid has no preconditions.
    let uid = unsafe { libc::getuid() };
    if !is_inbox_path(Path::new(socket), uid) {
        return Err(format!("Not a Claude Code inbox: {socket}"));
    }
    let mut stream = UnixStream::connect(socket).map_err(|e| e.to_string())?;
    stream.write_all(message_line(text).as_bytes()).map_err(|e| e.to_string())?;
    let _ = stream.shutdown(std::net::Shutdown::Write);
    Ok(())
}

#[cfg(not(unix))]
pub fn send(_socket: &str, _text: &str) -> Result<(), String> {
    Err("Not supported on this platform".into())
}

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
