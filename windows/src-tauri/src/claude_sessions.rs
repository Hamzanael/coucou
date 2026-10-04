// Running Claude Code sessions — interactive and background — from
// `claude agents --json`, falling back to the records Claude Code keeps in
// ~/.claude/sessions/<pid>.json. Hook events only reach Coucou while a session
// works; this also lists the quiet ones, and drops sessions that ended.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSession {
    pub pid: u32,
    pub session_id: String,
    /// Claude Code's own name for it (`--name`, `/rename`, or derived).
    pub name: String,
    pub cwd: String,
    pub busy: bool,
    /// A background agent blocked on the user.
    pub waiting: bool,
    pub background: bool,
    pub updated_at: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    pid: u32,
    session_id: Option<String>,
    name: Option<String>,
    cwd: Option<String>,
    status: Option<String>,
    updated_at: Option<u64>,
}

pub fn parse(json: &str) -> Option<LiveSession> {
    let r: Record = serde_json::from_str(json).ok()?;
    Some(LiveSession {
        pid: r.pid,
        session_id: r.session_id?,
        name: r.name.unwrap_or_default(),
        cwd: r.cwd.unwrap_or_default(),
        busy: r.status.as_deref() == Some("busy"),
        waiting: false,
        background: false,
        updated_at: r.updated_at.unwrap_or(0),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Agent {
    session_id: Option<String>,
    name: Option<String>,
    pid: Option<u32>,
    cwd: Option<String>,
    kind: Option<String>,
    status: Option<String>,
    state: Option<String>,
    started_at: Option<u64>,
}

/// `claude agents --json`: interactive sessions carry `status` (busy / idle),
/// background ones `state` (working / blocked / …).
pub fn parse_agents(json: &str) -> Option<Vec<LiveSession>> {
    let agents: Vec<Agent> = serde_json::from_str(json).ok()?;
    Some(
        agents
            .into_iter()
            .filter_map(|a| {
                let state = a.state.as_deref().unwrap_or("");
                Some(LiveSession {
                    pid: a.pid.unwrap_or(0),
                    session_id: a.session_id?,
                    name: a.name.unwrap_or_default(),
                    cwd: a.cwd.unwrap_or_default(),
                    busy: a.status.as_deref() == Some("busy") || matches!(state, "working" | "running"),
                    waiting: state == "blocked",
                    background: a.kind.as_deref() == Some("background"),
                    updated_at: a.started_at.unwrap_or(0),
                })
            })
            .collect(),
    )
}

/// A record outlives a crashed session: only trust it while its process runs.
fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|c| c.trim() == "claude")
}

/// GNOME starts Coucou without the shell's PATH, so ~/.local/bin (where the
/// Claude Code installer puts `claude`) is checked first.
fn claude_bin() -> std::path::PathBuf {
    let local = crate::platform::home().join(".local/bin/claude");
    if local.is_file() {
        return local;
    }
    crate::find_on_path("claude").unwrap_or_else(|| "claude".into())
}

fn from_cli() -> Option<Vec<LiveSession>> {
    let out = std::process::Command::new(claude_bin()).args(["agents", "--json"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_agents(&String::from_utf8_lossy(&out.stdout))
}

pub fn running() -> Vec<LiveSession> {
    if let Some(mut list) = from_cli() {
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        return list;
    }
    from_records()
}

fn from_records() -> Vec<LiveSession> {
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
        // Each poll starts the Claude CLI, so not too often.
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(20));
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
    fn parses_claude_agents_json_including_background_agents() {
        let json = r#"[
          {"id":"4047a5f6","cwd":"/p/middleware","kind":"background","sessionId":"4047a5f6-e","name":"modee","state":"blocked","startedAt":5},
          {"cwd":"/p/analytickBE","kind":"interactive","sessionId":"efa7","name":"be","status":"busy","pid":61134,"startedAt":9},
          {"cwd":"/p/x","kind":"interactive","name":"no-id"}
        ]"#;
        let list = parse_agents(json).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].background && list[0].waiting && !list[0].busy);
        assert!(!list[1].background && list[1].busy && !list[1].waiting);
        assert_eq!(list[1].pid, 61134);
        assert_eq!(list[0].name, "modee");
        assert_eq!(list[1].name, "be");
        assert!(parse_agents("garbage").is_none());
    }

    #[test]
    fn hides_background_agents_that_are_long_gone() {
        let day = 86_400_000u64;
        let now = 100 * day;
        let json = format!(r#"[
          {{"kind":"background","sessionId":"old","state":"blocked","startedAt":{}}},
          {{"kind":"background","sessionId":"recent","state":"blocked","startedAt":{}}},
          {{"kind":"background","sessionId":"working","state":"working","startedAt":{}}}
        ]"#, now - 48 * day, now - day, now - 48 * day);
        let ids: Vec<String> = current(parse_agents(&json).unwrap(), now).into_iter().map(|s| s.session_id).collect();
        assert_eq!(ids, vec!["recent".to_string(), "working".to_string()]);
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
