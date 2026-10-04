#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use zbus::zvariant::{OwnedValue, Value};

    fn hints(target: u64, action: &str) -> HashMap<String, OwnedValue> {
        let mut h = HashMap::new();
        h.insert("default-action".to_string(), OwnedValue::try_from(Value::from(action)).unwrap());
        h.insert("default-action-target".to_string(), OwnedValue::try_from(Value::U64(target)).unwrap());
        h
    }

    #[test]
    fn reads_the_surface_from_ghosttys_own_notification() {
        let h = hints(108923765784736, "app.present-surface");
        assert_eq!(surface_from_notification("com.mitchellh.ghostty", "Coucou 7f3a", &h, "7f3a"), Some(108923765784736));
        assert_eq!(surface_from_notification("org.other.app", "Coucou 7f3a", &h, "7f3a"), None, "another app");
        assert_eq!(surface_from_notification("com.mitchellh.ghostty", "Coucou 0000", &h, "7f3a"), None, "another request");
        let wrong = hints(5, "app.new-window");
        assert_eq!(surface_from_notification("com.mitchellh.ghostty", "Coucou 7f3a", &wrong, "7f3a"), None);
    }

    #[test]
    fn writes_an_osc_777_notify() {
        assert_eq!(osc_notify("Coucou 7f3a"), b"\x1b]777;notify;Coucou;Coucou 7f3a\x07".to_vec());
    }

    #[test]
    fn only_pseudo_terminals_are_written_to() {
        assert!(is_pty(std::path::Path::new("/dev/pts/4")));
        assert!(!is_pty(std::path::Path::new("/dev/null")));
        assert!(!is_pty(std::path::Path::new("/home/u/file")));
    }
}
