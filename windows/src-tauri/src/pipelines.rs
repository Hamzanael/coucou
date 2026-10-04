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
