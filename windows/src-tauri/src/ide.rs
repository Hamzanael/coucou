// Opens a project in IntelliJ: the JetBrains Toolbox launcher script first,
// then the snap packages, then an `idea` on PATH.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The first candidate that exists, else whatever PATH offers.
pub fn resolve(candidates: &[PathBuf], on_path: Option<PathBuf>) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned().or(on_path)
}

fn candidates() -> Vec<PathBuf> {
    let home = crate::platform::home();
    vec![
        home.join(".local/share/JetBrains/Toolbox/scripts/idea"),
        PathBuf::from("/snap/bin/intellij-idea-ultimate"),
        PathBuf::from("/snap/bin/intellij-idea-community"),
    ]
}

/// Opens `path` in IntelliJ. False when no launcher was found or it failed to start.
pub fn open(path: Option<&str>) -> bool {
    let Some(launcher) = resolve(&candidates(), crate::find_on_path("idea")) else { return false };
    let mut cmd = Command::new(launcher);
    if let Some(p) = path.filter(|p| !p.is_empty() && Path::new(p).is_dir()) {
        cmd.arg(p);
    }
    crate::platform::quiet(&mut cmd).spawn().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_an_existing_candidate_over_path() {
        let dir = std::env::temp_dir().join(format!("coucou-ide-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("idea");
        std::fs::write(&script, "#!/bin/sh\n").unwrap();
        let got = resolve(&[dir.join("missing"), script.clone()], Some(PathBuf::from("/usr/bin/idea")));
        assert_eq!(got, Some(script));
    }

    #[test]
    fn falls_back_to_path_then_none() {
        assert_eq!(
            resolve(&[PathBuf::from("/nope/idea")], Some(PathBuf::from("/p/idea"))),
            Some(PathBuf::from("/p/idea"))
        );
        assert_eq!(resolve(&[PathBuf::from("/nope/idea")], None), None);
    }
}
