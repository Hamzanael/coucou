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
