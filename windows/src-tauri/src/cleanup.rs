// Free-up actions offered by the Health view. Each is a fixed, named operation —
// the page sends only an id, never a path or a command — sized before it is
// offered and run only on an explicit click. Docker volumes are never touched:
// they hold databases.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

pub enum Kind {
    DockerContainers,
    DockerImages,
    /// A cache directory under $HOME that tools rebuild on their own.
    Dir(&'static str),
    /// IntelliJ's caches: only while IntelliJ is closed.
    IntellijCaches,
    /// Needs root: shown for the user to run, never run here.
    Command(&'static str),
}

pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    pub note: &'static str,
    pub kind: Kind,
}

pub static ACTIONS: &[Action] = &[
    Action {
        id: "docker-containers",
        label: "Docker stopped containers",
        note: "Their data is gone for good; running ones are kept",
        kind: Kind::DockerContainers,
    },
    Action {
        id: "docker-images",
        label: "Docker unused images",
        note: "Pulled again when needed",
        kind: Kind::DockerImages,
    },
    Action { id: "gradle", label: "Gradle caches", note: "Re-downloaded on the next build", kind: Kind::Dir(".gradle/caches") },
    Action { id: "npm", label: "npm cache", note: "Re-downloaded on the next install", kind: Kind::Dir(".npm/_cacache") },
    Action {
        id: "playwright",
        label: "Playwright browsers",
        note: "Reinstalled with `npx playwright install`",
        kind: Kind::Dir(".cache/ms-playwright"),
    },
    Action {
        id: "intellij-caches",
        label: "IntelliJ caches",
        note: "IntelliJ re-indexes on next start; close it first",
        kind: Kind::IntellijCaches,
    },
    Action {
        id: "journal",
        label: "System journal",
        note: "Keeps the newest 500 MB of logs",
        kind: Kind::Command("sudo journalctl --vacuum-size=500M"),
    },
    Action {
        id: "snaps",
        label: "Old snap revisions",
        note: "Disabled revisions only",
        kind: Kind::Command(
            "snap list --all | awk '/disabled/{print $1, $3}' | while read n r; do sudo snap remove \"$n\" --revision=\"$r\"; done",
        ),
    },
];

pub fn action(id: &str) -> Option<&'static Action> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// Docker's SI sizes ("6.219GB", "57.01GB (99%)", "0B").
pub fn parse_size(s: &str) -> Option<u64> {
    let token = s.split_whitespace().next()?;
    let split = token.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = token.split_at(split);
    let n: f64 = num.parse().ok()?;
    let mult = match unit.to_ascii_uppercase().as_str() {
        "B" => 1.0,
        "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        _ => return None,
    };
    Some((n * mult).round() as u64)
}

/// `docker system df --format '{{json .}}'` → reclaimable bytes per type.
pub fn docker_reclaimable(json_lines: &str) -> HashMap<String, u64> {
    json_lines
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| {
            let kind = v.get("Type")?.as_str()?.to_string();
            let size = parse_size(v.get("Reclaimable")?.as_str()?)?;
            Some((kind, size))
        })
        .collect()
}

pub fn intellij_running(cmdlines: &[String]) -> bool {
    cmdlines.iter().any(|c| c.contains("com.intellij.idea.Main"))
}

fn cmdlines() -> Vec<String> {
    let Ok(dir) = std::fs::read_dir("/proc") else { return Vec::new() };
    dir.flatten()
        .filter(|e| e.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit()))
        .filter_map(|e| std::fs::read(e.path().join("cmdline")).ok())
        .map(|b| String::from_utf8_lossy(&b).replace('\0', " "))
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub label: String,
    pub note: String,
    pub size_bytes: Option<u64>,
    /// For root-only items: the command to copy.
    pub command: Option<String>,
    /// False when it cannot run right now (nothing to free, IntelliJ open…).
    pub available: bool,
    pub detail: Option<String>,
}

fn home_dir(rel: &str) -> PathBuf {
    crate::platform::home().join(rel)
}

fn du_bytes(path: &PathBuf) -> Option<u64> {
    let out = Command::new("du").args(["-sb", "--one-file-system"]).arg(path).output().ok()?;
    String::from_utf8_lossy(&out.stdout).split_whitespace().next()?.parse().ok()
}

fn docker(args: &[&str]) -> Option<String> {
    let out = Command::new("docker").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn scan() -> Vec<Item> {
    let df = docker(&["system", "df", "--format", "{{json .}}"]).map(|s| docker_reclaimable(&s)).unwrap_or_default();
    let ide_open = intellij_running(&cmdlines());
    ACTIONS
        .iter()
        .map(|a| {
            let (size, available, detail, command) = match &a.kind {
                Kind::DockerContainers => {
                    let names = docker(&["ps", "-a", "--filter", "status=exited", "--filter", "status=created", "--format", "{{.Names}}"])
                        .map(|s| s.lines().map(str::to_string).collect::<Vec<_>>())
                        .unwrap_or_default();
                    let size = df.get("Containers").copied();
                    (size, !names.is_empty(), (!names.is_empty()).then(|| names.join(", ")), None)
                }
                Kind::DockerImages => {
                    let size = df.get("Images").copied();
                    (size, size.is_some_and(|s| s > 0), None, None)
                }
                Kind::Dir(rel) => {
                    let p = home_dir(rel);
                    let size = p.is_dir().then(|| du_bytes(&p)).flatten();
                    (size, size.is_some_and(|s| s > 0), None, None)
                }
                Kind::IntellijCaches => {
                    let p = home_dir(".cache/JetBrains");
                    let size = p.is_dir().then(|| du_bytes(&p)).flatten();
                    let detail = ide_open.then(|| "IntelliJ is running — close it to free this".to_string());
                    (size, !ide_open && size.is_some_and(|s| s > 0), detail, None)
                }
                Kind::Command(cmd) => (None, true, None, Some(cmd.to_string())),
            };
            Item {
                id: a.id.into(),
                label: a.label.into(),
                note: a.note.into(),
                size_bytes: size,
                command,
                available,
                detail,
            }
        })
        .collect()
}

fn remove_cache_dir(rel: &str) -> Result<String, String> {
    let p = home_dir(rel);
    if !p.is_dir() {
        return Ok("Already empty".into());
    }
    let size = du_bytes(&p).unwrap_or(0);
    std::fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(format!("Freed {:.1} GB", size as f64 / 1e9))
}

/// Runs one named action. The id is checked against the fixed list.
pub fn run(id: &str) -> Result<String, String> {
    let a = action(id).ok_or_else(|| format!("Unknown action {id}"))?;
    let reclaimed = |out: Option<String>| {
        out.and_then(|s| s.lines().rev().find(|l| l.contains("reclaimed")).map(str::to_string))
            .unwrap_or_else(|| "Done".into())
    };
    match &a.kind {
        Kind::DockerContainers => Ok(reclaimed(docker(&["container", "prune", "-f"]))),
        Kind::DockerImages => Ok(reclaimed(docker(&["image", "prune", "-a", "-f"]))),
        Kind::Dir(rel) => remove_cache_dir(rel),
        Kind::IntellijCaches => {
            if intellij_running(&cmdlines()) {
                return Err("Close IntelliJ first".into());
            }
            remove_cache_dir(".cache/JetBrains")
        }
        Kind::Command(_) => Err("Needs sudo: copy the command and run it in a terminal".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_docker_sizes() {
        assert_eq!(parse_size("6.219GB"), Some(6_219_000_000));
        assert_eq!(parse_size("394.6MB"), Some(394_600_000));
        assert_eq!(parse_size("0B"), Some(0));
        assert_eq!(parse_size("57.01GB (99%)"), Some(57_010_000_000));
        assert_eq!(parse_size("nonsense"), None);
    }

    #[test]
    fn reads_reclaimable_from_docker_system_df() {
        let df = r#"{"Active":"17","Reclaimable":"6.219GB (36%)","Size":"16.84GB","TotalCount":"37","Type":"Images"}
{"Active":"6","Reclaimable":"57.01GB (99%)","Size":"57.01GB","TotalCount":"18","Type":"Containers"}
{"Active":"13","Reclaimable":"9.02GB (26%)","Size":"34.02GB","TotalCount":"60","Type":"Local Volumes"}"#;
        let r = docker_reclaimable(df);
        assert_eq!(r.get("Images"), Some(&6_219_000_000));
        assert_eq!(r.get("Containers"), Some(&57_010_000_000));
    }

    #[test]
    fn detects_a_running_intellij() {
        assert!(intellij_running(&["/opt/jdk/bin/java -cp x com.intellij.idea.Main".into()]));
        assert!(!intellij_running(&["/usr/bin/node server.js".into()]));
    }

    #[test]
    fn only_known_actions_can_run_and_volumes_are_never_one() {
        let ids: Vec<&str> = ACTIONS.iter().map(|a| a.id).collect();
        assert!(ids.contains(&"docker-containers"));
        assert!(ids.contains(&"gradle"));
        assert!(!ids.iter().any(|id| id.contains("volume")));
        assert!(action("rm-rf-home").is_none());
    }
}
