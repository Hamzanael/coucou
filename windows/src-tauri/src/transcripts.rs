// What each Claude Code session is about, from the tail of its transcript
// (~/.claude/projects/<cwd as dashes>/<session id>.jsonl): the tab title Claude
// gives it (ai-title), your last prompt and Claude's last written reply.

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

/// Transcripts run to megabytes; the latest entries are all within this much.
const TAIL_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Insight {
    pub title: Option<String>,
    pub last_prompt: Option<String>,
    pub last_reply: Option<String>,
}

fn clip(s: &str, max: usize) -> String {
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= max {
        s
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// "[Image #9]" placeholders say nothing out of context.
fn clean_prompt(p: &str) -> String {
    let mut out = String::new();
    let mut rest = p;
    while let Some(start) = rest.find("[Image #") {
        out.push_str(&rest[..start]);
        match rest[start..].find(']') {
            Some(end) => rest = rest[start + end + 1..].trim_start(),
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    clip(&out, 140)
}

pub fn parse_tail(text: &str) -> Insight {
    let mut insight = Insight::default();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        match v.get("type").and_then(Value::as_str) {
            Some("ai-title") => {
                if let Some(t) = v.get("aiTitle").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
                    insight.title = Some(clip(t, 60));
                }
            }
            Some("last-prompt") => {
                if let Some(p) = v.get("lastPrompt").and_then(Value::as_str).filter(|p| !p.trim().is_empty()) {
                    insight.last_prompt = Some(clean_prompt(p));
                }
            }
            Some("assistant") => {
                let text = v
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|c| c.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|c| c.get("text").and_then(Value::as_str))
                    .filter_map(|t| t.lines().map(str::trim).find(|l| !l.is_empty()))
                    .last();
                if let Some(t) = text {
                    insight.last_reply = Some(clip(t, 160));
                }
            }
            _ => {}
        }
    }
    insight
}

/// Claude Code's folder name for a cwd: every character but letters, digits
/// and '-' becomes '-'.
pub fn project_dir_name(cwd: &str) -> String {
    cwd.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect()
}

fn transcript(session_id: &str, cwd: &str) -> Option<PathBuf> {
    let projects = crate::platform::home().join(".claude").join("projects");
    let direct = projects.join(project_dir_name(cwd)).join(format!("{session_id}.jsonl"));
    if direct.is_file() {
        return Some(direct);
    }
    // A session started elsewhere and moved: look in every project folder.
    std::fs::read_dir(&projects)
        .ok()?
        .flatten()
        .map(|d| d.path().join(format!("{session_id}.jsonl")))
        .find(|p| p.is_file())
}

pub fn insight(session_id: &str, cwd: &str) -> Insight {
    let Some(path) = transcript(session_id, cwd) else { return Insight::default() };
    let Ok(mut file) = std::fs::File::open(path) else { return Insight::default() };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES)));
    let mut buf = Vec::new();
    let _ = file.read_to_end(&mut buf);
    parse_tail(&String::from_utf8_lossy(&buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAIL: &str = r#"{"type":"ai-title","aiTitle":"Old title"}
{"type":"user","message":{"role":"user","content":"hi"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"First reply\nsecond line"}]}}
{"type":"ai-title","aiTitle":"Coucou installation"}
{"type":"last-prompt","lastPrompt":"1- [Image #9] notch gets dropped down"}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}
half a line that was cut by the tail read"#;

    #[test]
    fn reads_the_latest_title_prompt_and_reply_from_a_tail() {
        let i = parse_tail(TAIL);
        assert_eq!(i.title.as_deref(), Some("Coucou installation"));
        assert_eq!(i.last_prompt.as_deref(), Some("1- notch gets dropped down"));
        assert_eq!(i.last_reply.as_deref(), Some("First reply"));
    }

    #[test]
    fn empty_or_garbage_gives_nothing() {
        let i = parse_tail("not json\n{}");
        assert!(i.title.is_none() && i.last_prompt.is_none() && i.last_reply.is_none());
    }

    #[test]
    fn transcript_folder_mirrors_the_cwd() {
        assert_eq!(project_dir_name("/home/u/IdeaProjects/analytickBE"), "-home-u-IdeaProjects-analytickBE");
        assert_eq!(project_dir_name("/home/u/a.b/.claude/x"), "-home-u-a-b--claude-x");
    }
}
