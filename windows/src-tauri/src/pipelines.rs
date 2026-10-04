// GitHub Actions status for the repos in Settings: the latest run of each
// workflow on main and on each open PR's branch, polled every 60 s with the
// GitHub token already stored for the GitHub pill. A new failure on main is
// reported once.

use std::collections::HashSet;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRow {
    pub repo: String,
    pub id: u64,
    pub workflow: String,
    pub branch: String,
    /// queued / in_progress / completed
    pub status: String,
    /// success / failure / cancelled / … once completed
    pub conclusion: Option<String>,
    pub url: String,
    pub created_at: String,
    pub pr_title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelinesUpdate {
    pub rows: Vec<RunRow>,
    pub new_failures: Vec<RunRow>,
    pub error: Option<String>,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// Runs arrive newest first; the first one seen per (workflow, branch) wins.
pub fn summarize(repo: &str, runs: &Value, open_prs: &[(String, String)]) -> Vec<RunRow> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in runs.get("workflow_runs").and_then(Value::as_array).into_iter().flatten() {
        let branch = s(r, "head_branch");
        let pr = open_prs.iter().find(|(b, _)| *b == branch);
        if branch != "main" && pr.is_none() {
            continue;
        }
        let workflow = s(r, "name");
        if !seen.insert((workflow.clone(), branch.clone())) {
            continue;
        }
        out.push(RunRow {
            repo: repo.to_string(),
            id: r.get("id").and_then(Value::as_u64).unwrap_or(0),
            workflow,
            branch,
            status: s(r, "status"),
            conclusion: r.get("conclusion").and_then(Value::as_str).map(str::to_string),
            url: s(r, "html_url"),
            created_at: s(r, "created_at"),
            pr_title: pr.map(|(_, t)| t.clone()),
        });
    }
    // main first, then PRs; each group keeps the API's newest-first order.
    out.sort_by_key(|r| r.branch != "main");
    out
}

pub fn new_main_failures(seen: &mut HashSet<u64>, rows: &[RunRow]) -> Vec<RunRow> {
    rows.iter()
        .filter(|r| r.branch == "main" && r.conclusion.as_deref() == Some("failure"))
        .filter(|r| seen.insert(r.id))
        .cloned()
        .collect()
}

async fn get(http: &reqwest::Client, token: &str, url: &str) -> Result<Value, String> {
    let response = http
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Coucou")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub {} for {url}", response.status().as_u16()));
    }
    response.json().await.map_err(|e| e.to_string())
}

async fn poll(http: &reqwest::Client, token: &str, repos: &[String]) -> Result<Vec<RunRow>, String> {
    let mut rows = Vec::new();
    for repo in repos {
        let prs = get(http, token, &format!("https://api.github.com/repos/{repo}/pulls?state=open&per_page=30")).await?;
        let open: Vec<(String, String)> = prs
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| (p.pointer("/head/ref").and_then(Value::as_str).unwrap_or_default().to_string(), s(p, "title")))
            .collect();
        let runs = get(http, token, &format!("https://api.github.com/repos/{repo}/actions/runs?per_page=60")).await?;
        rows.extend(summarize(repo, &runs, &open));
    }
    Ok(rows)
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let http = reqwest::Client::builder().timeout(Duration::from_secs(15)).build().unwrap_or_default();
        let mut seen: HashSet<u64> = HashSet::new();
        let mut first = true;
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            if crate::integrations::PAUSED.load(std::sync::atomic::Ordering::Relaxed) {
                continue;
            }
            let repos = app
                .try_state::<crate::Shared>()
                .map(|s| s.settings.lock().unwrap().pipeline_repos.clone())
                .unwrap_or_default();
            let Some(token) = crate::secrets::get("github-token") else { continue };
            if repos.is_empty() {
                continue;
            }
            let update = match poll(&http, &token, &repos).await {
                Ok(rows) => {
                    let fresh = new_main_failures(&mut seen, &rows);
                    // Failures already there at launch fill the list, not the alerts.
                    let new_failures = if first { Vec::new() } else { fresh };
                    first = false;
                    PipelinesUpdate { rows, new_failures, error: None }
                }
                Err(e) => PipelinesUpdate { rows: Vec::new(), new_failures: Vec::new(), error: Some(e) },
            };
            let _ = app.emit_to(crate::island::WINDOW_LABEL, "pipelines", update);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(id: u64, wf: &str, branch: &str, status: &str, conclusion: Option<&str>) -> serde_json::Value {
        json!({"id": id, "name": wf, "head_branch": branch, "status": status, "conclusion": conclusion,
               "html_url": format!("https://github.com/x/y/actions/runs/{id}"), "created_at": "2026-10-04T09:43:00Z",
               "display_title": format!("title {id}")})
    }

    #[test]
    fn keeps_the_latest_run_per_workflow_on_main_and_open_prs() {
        let runs = json!({"workflow_runs": [
            run(9, "CI", "main", "in_progress", None),
            run(8, "CI", "main", "completed", Some("success")),
            run(7, "Publish", "main", "completed", Some("failure")),
            run(6, "CI", "feature-a", "completed", Some("success")),
            run(5, "CI", "old-branch", "completed", Some("failure")),
        ]});
        let prs = vec![("feature-a".to_string(), "Add login".to_string())];
        let rows = summarize("o/app", &runs, &prs);
        let keys: Vec<(String, String, u64)> = rows.iter().map(|r| (r.workflow.clone(), r.branch.clone(), r.id)).collect();
        assert_eq!(keys, vec![
            ("CI".into(), "main".into(), 9),
            ("Publish".into(), "main".into(), 7),
            ("CI".into(), "feature-a".into(), 6),
        ]);
        assert_eq!(rows[2].pr_title.as_deref(), Some("Add login"));
        assert_eq!(rows[0].repo, "o/app");
    }

    #[test]
    fn reports_each_main_failure_once() {
        let runs = json!({"workflow_runs": [run(7, "Publish", "main", "completed", Some("failure")),
                                            run(6, "CI", "pr", "completed", Some("failure"))]});
        let rows = summarize("o/app", &runs, &[("pr".into(), "t".into())]);
        let mut seen = std::collections::HashSet::new();
        let first = new_main_failures(&mut seen, &rows);
        assert_eq!(first.iter().map(|r| r.id).collect::<Vec<_>>(), vec![7]);
        assert!(new_main_failures(&mut seen, &rows).is_empty());
    }
}
