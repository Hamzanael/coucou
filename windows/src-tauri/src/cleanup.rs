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
