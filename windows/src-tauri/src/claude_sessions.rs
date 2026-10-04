// Running Claude Code sessions, from the records Claude Code keeps in
// ~/.claude/sessions/<pid>.json. Hook events only reach Coucou while a session
// works; this also lists the quiet ones, and drops sessions whose process ended.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSession {
    pub pid: u32,
    pub session_id: String,
    pub cwd: String,
    pub busy: bool,
    pub updated_at: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    pid: u32,
    session_id: Option<String>,
    cwd: Option<String>,
    status: Option<String>,
    updated_at: Option<u64>,
}

pub fn parse(json: &str) -> Option<LiveSession> {
    let r: Record = serde_json::from_str(json).ok()?;
    Some(LiveSession {
        pid: r.pid,
        session_id: r.session_id?,
        cwd: r.cwd.unwrap_or_default(),
        busy: r.status.as_deref() == Some("busy"),
        updated_at: r.updated_at.unwrap_or(0),
    })
}

/// A record outlives a crashed session: only trust it while its process runs.
fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|c| c.trim() == "claude")
}

pub fn running() -> Vec<LiveSession> {
    let dir = crate::platform::home().join(".claude").join("sessions");
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<LiveSession> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| parse(&std::fs::read_to_string(e.path()).ok()?))
        .filter(|s| alive(s.pid))
        .collect();
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    out
}

/// Sends `claude-sessions` whenever the set of running sessions changes.
#[cfg(target_os = "linux")]
pub fn start(app: tauri::AppHandle) {
    use tauri::Emitter;
    tauri::async_runtime::spawn(async move {
        let mut last: Option<Vec<LiveSession>> = None;
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            ticker.tick().await;
            let now = tauri::async_runtime::spawn_blocking(running).await.unwrap_or_default();
            if last.as_ref() != Some(&now) {
                let _ = app.emit_to(crate::island::WINDOW_LABEL, "claude-sessions", &now);
                last = Some(now);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECORD: &str = r#"{"pid":61134,"sessionId":"efa7021c","cwd":"/home/u/IdeaProjects/analytickBE","name":"analytickbe-6e","status":"idle","updatedAt":1791106970612,"kind":"interactive"}"#;

    #[test]
    fn parses_a_session_record() {
        let s = parse(RECORD).unwrap();
        assert_eq!(s.session_id, "efa7021c");
        assert_eq!(s.cwd, "/home/u/IdeaProjects/analytickBE");
        assert_eq!(s.busy, false);
        assert_eq!(s.updated_at, 1791106970612);
        assert_eq!(s.pid, 61134);
    }

    #[test]
    fn busy_status_and_missing_fields() {
        let s = parse(r#"{"pid":1,"sessionId":"x","cwd":"/a","status":"busy"}"#).unwrap();
        assert!(s.busy);
        assert_eq!(s.updated_at, 0);
        assert!(parse(r#"{"pid":1,"cwd":"/a"}"#).is_none(), "no sessionId → not a session");
        assert!(parse("not json").is_none());
    }
}
