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
