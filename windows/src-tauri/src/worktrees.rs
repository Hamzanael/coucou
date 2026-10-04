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

    #[test]
    fn remove_refuses_paths_not_in_the_scan() {
        let r = remove(&[], &["/etc".to_string()]);
        assert_eq!(r.len(), 1);
        assert!(!r[0].ok);
    }
}
