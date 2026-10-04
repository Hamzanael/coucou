// Stale git worktrees under the configured roots: missing, merged / upstream
// gone, or clean and idle. Local refs only — no fetch, no network. Removal is
// `git worktree remove` without --force, then prune; branches are never touched.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub branch: Option<String>,
    pub prunable: bool,
}

pub fn parse_porcelain(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for block in text.split("\n\n").filter(|b| !b.trim().is_empty()) {
        let mut entry = Entry { path: String::new(), branch: None, prunable: false };
        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                entry.path = p.to_string();
            } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                entry.branch = Some(b.to_string());
            } else if line.starts_with("prunable") {
                entry.prunable = true;
            }
        }
        if !entry.path.is_empty() {
            out.push(entry);
        }
    }
    out
}

pub struct Facts {
    pub exists: bool,
    pub merged: bool,
    pub upstream_gone: bool,
    pub clean: bool,
    pub idle_days: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    Missing,
    Merged,
    UpstreamGone,
    Idle,
}

pub fn classify(f: &Facts, idle_days: u64) -> Option<Reason> {
    if !f.exists {
        Some(Reason::Missing)
    } else if f.merged {
        Some(Reason::Merged)
    } else if f.upstream_gone {
        Some(Reason::UpstreamGone)
    } else if f.clean && f.idle_days.is_some_and(|d| d >= idle_days) {
        Some(Reason::Idle)
    } else {
        None
    }
}

pub fn pick_default_branch(origin_head: Option<&str>, local: &[String]) -> String {
    if let Some(h) = origin_head {
        return h.to_string();
    }
    for name in ["main", "master"] {
        if local.iter().any(|b| b == name) {
            return name.to_string();
        }
    }
    "HEAD".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stale {
    pub repo: String,
    pub path: String,
    pub branch: Option<String>,
    pub reason: Reason,
    pub idle_days: Option<u64>,
    pub size_kb: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Removal {
    pub path: String,
    pub ok: bool,
    pub message: String,
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

fn lines(s: Option<String>) -> Vec<String> {
    s.map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

/// Repos directly in each root (a `.git` directory — worktrees have a `.git` file).
fn repos(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        if root.join(".git").is_dir() {
            out.push(root.clone());
        }
        let Ok(dir) = std::fs::read_dir(root) else { continue };
        for e in dir.flatten() {
            if e.path().join(".git").is_dir() {
                out.push(e.path());
            }
        }
    }
    out
}

fn days_since(t: SystemTime) -> Option<u64> {
    SystemTime::now().duration_since(t).ok().map(|d| d.as_secs() / 86_400)
}

/// Last activity: the newer of the HEAD commit and the worktree's own git dir
/// (checkout / commit / index refresh), so a fresh worktree of an old commit is
/// not "idle" on day one.
fn idle_days(path: &Path) -> Option<u64> {
    let commit = git(path, &["log", "-1", "--format=%ct"])
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|secs| SystemTime::UNIX_EPOCH + Duration::from_secs(secs));
    let gitdir = git(path, &["rev-parse", "--absolute-git-dir"]).map(|s| PathBuf::from(s.trim()));
    let touched = gitdir
        .iter()
        .flat_map(|d| ["HEAD", "index"].map(|f| d.join(f)))
        .filter_map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok())
        .max();
    commit.into_iter().chain(touched).max().and_then(days_since)
}

fn size_kb(path: &Path) -> Option<u64> {
    let out = Command::new("du").args(["-sk", "--one-file-system"]).arg(path).output().ok()?;
    String::from_utf8_lossy(&out.stdout).split_whitespace().next()?.parse().ok()
}

pub fn scan(roots: &[PathBuf], idle_threshold: u64) -> Vec<Stale> {
    let mut out = Vec::new();
    for repo in repos(roots) {
        let entries = parse_porcelain(&git(&repo, &["worktree", "list", "--porcelain"]).unwrap_or_default());
        if entries.len() < 2 {
            continue;
        }
        let local = lines(git(&repo, &["for-each-ref", "--format=%(refname:short)", "refs/heads"]));
        let origin_head = git(&repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
            .map(|s| s.trim().to_string());
        let default = pick_default_branch(origin_head.as_deref(), &local);
        let merged = lines(git(&repo, &["branch", "--merged", &default, "--format=%(refname:short)"]));
        let gone: Vec<String> = lines(git(
            &repo,
            &["for-each-ref", "--format=%(refname:short) %(upstream:track)", "refs/heads"],
        ))
        .into_iter()
        .filter(|l| l.ends_with("[gone]"))
        .filter_map(|l| l.split(' ').next().map(str::to_string))
        .collect();
        // The first entry is the main worktree: never a candidate.
        for e in entries.into_iter().skip(1) {
            let path = PathBuf::from(&e.path);
            let exists = !e.prunable && path.is_dir();
            let on = |list: &[String]| e.branch.as_ref().is_some_and(|b| list.contains(b));
            let facts = Facts {
                exists,
                merged: on(&merged),
                upstream_gone: on(&gone),
                clean: exists && git(&path, &["status", "--porcelain"]).is_some_and(|s| s.trim().is_empty()),
                idle_days: if exists { idle_days(&path) } else { None },
            };
            if let Some(reason) = classify(&facts, idle_threshold) {
                out.push(Stale {
                    repo: repo.to_string_lossy().to_string(),
                    path: e.path.clone(),
                    branch: e.branch.clone(),
                    reason,
                    idle_days: facts.idle_days,
                    size_kb: if exists { size_kb(&path) } else { None },
                });
            }
        }
    }
    out
}

/// Removes only what the last scan reported, never with --force: a worktree
/// with changes is refused by git and reported back.
pub fn remove(stale: &[Stale], paths: &[String]) -> Vec<Removal> {
    paths
        .iter()
        .map(|p| {
            let Some(s) = stale.iter().find(|s| &s.path == p) else {
                return Removal { path: p.clone(), ok: false, message: "Not in the last scan".into() };
            };
            let repo = Path::new(&s.repo);
            let result = if s.reason == Reason::Missing {
                Command::new("git").arg("-C").arg(repo).args(["worktree", "prune"]).output()
            } else {
                Command::new("git").arg("-C").arg(repo).args(["worktree", "remove"]).arg(&s.path).output()
            };
            let _ = Command::new("git").arg("-C").arg(repo).args(["worktree", "prune"]).output();
            match result {
                Ok(o) if o.status.success() => Removal { path: p.clone(), ok: true, message: "Removed".into() },
                Ok(o) => Removal {
                    path: p.clone(),
                    ok: false,
                    message: String::from_utf8_lossy(&o.stderr).trim().to_string(),
                },
                Err(e) => Removal { path: p.clone(), ok: false, message: e.to_string() },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORCELAIN: &str = "worktree /home/u/IdeaProjects/app\nHEAD 1111\nbranch refs/heads/main\n\n\
worktree /home/u/IdeaProjects/app/.claude/worktrees/fix login\nHEAD 2222\nbranch refs/heads/claude/fix-login\n\n\
worktree /tmp/gone-wt\nHEAD 3333\ndetached\nprunable gitdir file points to non-existent location\n";

    #[test]
    fn parses_entries_including_spaces_and_prunable() {
        let e = parse_porcelain(PORCELAIN);
        assert_eq!(e.len(), 3);
        assert_eq!(e[1].path, "/home/u/IdeaProjects/app/.claude/worktrees/fix login");
        assert_eq!(e[1].branch.as_deref(), Some("claude/fix-login"));
        assert!(e[2].prunable);
        assert_eq!(e[2].branch, None);
    }

    fn facts() -> Facts {
        Facts { exists: true, merged: false, upstream_gone: false, clean: true, idle_days: Some(1) }
    }

    #[test]
    fn classifies_by_priority() {
        assert_eq!(classify(&Facts { exists: false, ..facts() }, 14), Some(Reason::Missing));
        assert_eq!(classify(&Facts { merged: true, clean: false, ..facts() }, 14), Some(Reason::Merged));
        assert_eq!(classify(&Facts { upstream_gone: true, ..facts() }, 14), Some(Reason::UpstreamGone));
        assert_eq!(classify(&Facts { idle_days: Some(20), ..facts() }, 14), Some(Reason::Idle));
        assert_eq!(classify(&Facts { idle_days: Some(20), clean: false, ..facts() }, 14), None);
        assert_eq!(classify(&facts(), 14), None);
    }

    #[test]
    fn default_branch_falls_back() {
        assert_eq!(pick_default_branch(Some("origin/develop"), &["main".into()]), "origin/develop");
        assert_eq!(pick_default_branch(None, &["master".into(), "x".into()]), "master");
        assert_eq!(pick_default_branch(None, &["main".into(), "master".into()]), "main");
        assert_eq!(pick_default_branch(None, &[]), "HEAD");
    }

    fn stale(path: &str, reason: Reason, idle_days: Option<u64>) -> Stale {
        Stale { repo: "/r".into(), path: path.into(), branch: None, reason, idle_days, size_kb: None }
    }

    #[test]
    fn auto_clean_takes_only_idle_past_the_threshold_and_missing() {
        let found = vec![
            stale("/a", Reason::Idle, Some(7)),
            stale("/b", Reason::Idle, Some(6)),
            stale("/c", Reason::Merged, Some(30)),
            stale("/d", Reason::UpstreamGone, Some(30)),
            stale("/e", Reason::Missing, None),
        ];
        assert_eq!(auto_candidates(&found, 7), vec!["/a".to_string(), "/e".to_string()]);
    }

    #[test]
    fn remove_refuses_paths_not_in_the_scan() {
        let r = remove(&[], &["/etc".to_string()]);
        assert_eq!(r.len(), 1);
        assert!(!r[0].ok);
    }
}
