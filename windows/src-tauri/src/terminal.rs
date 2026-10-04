// Takes you to the Ghostty tab a Claude Code session runs in. Ghostty presents
// a tab by its surface id (org.gtk.Actions `present-surface`) but never lists
// ids — except inside the desktop notifications it raises. So the first time,
// Coucou writes an OSC 777 notify to the session's terminal, reads the id off
// Ghostty's AddNotification on the session bus, removes that notification and
// presents the tab. Later clicks reuse the id until Ghostty restarts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use zbus::zvariant::{OwnedValue, Value};

const GHOSTTY: &str = "com.mitchellh.ghostty";

/// (Ghostty pid, session pid) → surface id.
static SURFACES: LazyLock<Mutex<HashMap<(u32, u32), u64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn osc_notify(body: &str) -> Vec<u8> {
    format!("\x1b]777;notify;Coucou;{body}\x07").into_bytes()
}

pub fn is_pty(path: &Path) -> bool {
    path.starts_with("/dev/pts/")
        && path.file_name().is_some_and(|n| !n.is_empty() && n.to_string_lossy().chars().all(|c| c.is_ascii_digit()))
}

/// The surface id in Ghostty's AddNotification(app, id, hints) for our request.
pub fn surface_from_notification(app: &str, id: &str, hints: &HashMap<String, OwnedValue>, nonce: &str) -> Option<u64> {
    if app != GHOSTTY || !id.contains(nonce) {
        return None;
    }
    match hints.get("default-action").map(|v| &**v) {
        Some(Value::Str(s)) if s.as_str() == "app.present-surface" => {}
        _ => return None,
    }
    match hints.get("default-action-target").map(|v| &**v) {
        Some(Value::U64(t)) => Some(*t),
        _ => None,
    }
}

fn parent(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // pid (comm) state ppid … — comm may hold spaces, so split after the ')'.
    stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse().ok()
}

fn comm(pid: u32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default().trim().to_string()
}

fn ghostty_of(mut pid: u32) -> Option<u32> {
    for _ in 0..8 {
        pid = parent(pid)?;
        if pid <= 1 {
            return None;
        }
        if comm(pid) == "ghostty" {
            return Some(pid);
        }
    }
    None
}

fn tty_of(pid: u32) -> Option<PathBuf> {
    let tty = std::fs::read_link(format!("/proc/{pid}/fd/0")).ok()?;
    is_pty(&tty).then_some(tty)
}

async fn present(conn: &zbus::Connection, surface: u64) -> zbus::Result<()> {
    conn.call_method(
        Some(GHOSTTY),
        "/com/mitchellh/ghostty",
        Some("org.gtk.Actions"),
        "Activate",
        &("present-surface", vec![Value::U64(surface)], HashMap::<&str, Value>::new()),
    )
    .await
    .map(|_| ())
}

async fn learn_surface(tty: &Path, nonce: &str) -> Result<u64, String> {
    let monitor = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::MethodCall)
        .interface("org.gtk.Notifications")
        .and_then(|b| b.member("AddNotification"))
        .map_err(|e| e.to_string())?
        .build();
    zbus::fdo::MonitoringProxy::new(&monitor)
        .await
        .map_err(|e| e.to_string())?
        .become_monitor(&[rule], 0)
        .await
        .map_err(|e| e.to_string())?;
    let mut stream = zbus::MessageStream::from(&monitor);

    std::fs::OpenOptions::new()
        .write(true)
        .open(tty)
        .and_then(|mut f| std::io::Write::write_all(&mut f, &osc_notify(&format!("Coucou {nonce}"))))
        .map_err(|e| e.to_string())?;

    let found = tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(Ok(msg)) = stream.next().await {
            let Ok((app, id, hints)) = msg.body().deserialize::<(String, String, HashMap<String, OwnedValue>)>() else {
                continue;
            };
            if let Some(surface) = surface_from_notification(&app, &id, &hints, nonce) {
                return Some((surface, id));
            }
        }
        None
    })
    .await
    .ok()
    .flatten();
    let (surface, id) = found.ok_or("Ghostty did not answer (desktop notifications off?)")?;

    if let Ok(conn) = zbus::Connection::session().await {
        let _ = conn
            .call_method(
                Some("org.gtk.Notifications"),
                "/org/gtk/Notifications",
                Some("org.gtk.Notifications"),
                "RemoveNotification",
                &(GHOSTTY, id.as_str()),
            )
            .await;
    }
    Ok(surface)
}

/// Brings the Ghostty tab running session `pid` to the front.
pub async fn focus(pid: u32) -> Result<String, String> {
    let ghostty = ghostty_of(pid).ok_or("This session is not running in Ghostty")?;
    let key = (ghostty, pid);
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    let cached = SURFACES.lock().unwrap().get(&key).copied();
    if let Some(surface) = cached {
        if present(&conn, surface).await.is_ok() {
            return Ok("Opened its terminal".into());
        }
    }
    let tty = tty_of(pid).ok_or("Could not find the session's terminal")?;
    let nonce = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() & 0xffff_ffff);
    let surface = learn_surface(&tty, &nonce).await?;
    SURFACES.lock().unwrap().insert(key, surface);
    present(&conn, surface).await.map_err(|e| e.to_string())?;
    Ok("Opened its terminal".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use zbus::zvariant::{OwnedValue, Value};

    fn hints(target: u64, action: &str) -> HashMap<String, OwnedValue> {
        let mut h = HashMap::new();
        h.insert("default-action".to_string(), OwnedValue::try_from(Value::from(action)).unwrap());
        h.insert("default-action-target".to_string(), OwnedValue::try_from(Value::U64(target)).unwrap());
        h
    }

    #[test]
    fn reads_the_surface_from_ghosttys_own_notification() {
        let h = hints(108923765784736, "app.present-surface");
        assert_eq!(surface_from_notification("com.mitchellh.ghostty", "Coucou 7f3a", &h, "7f3a"), Some(108923765784736));
        assert_eq!(surface_from_notification("org.other.app", "Coucou 7f3a", &h, "7f3a"), None, "another app");
        assert_eq!(surface_from_notification("com.mitchellh.ghostty", "Coucou 0000", &h, "7f3a"), None, "another request");
        let wrong = hints(5, "app.new-window");
        assert_eq!(surface_from_notification("com.mitchellh.ghostty", "Coucou 7f3a", &wrong, "7f3a"), None);
    }

    #[test]
    fn writes_an_osc_777_notify() {
        assert_eq!(osc_notify("Coucou 7f3a"), b"\x1b]777;notify;Coucou;Coucou 7f3a\x07".to_vec());
    }

    #[test]
    fn only_pseudo_terminals_are_written_to() {
        assert!(is_pty(std::path::Path::new("/dev/pts/4")));
        assert!(!is_pty(std::path::Path::new("/dev/null")));
        assert!(!is_pty(std::path::Path::new("/home/u/file")));
    }
}
