use std::fs;
use std::path::PathBuf;

pub(crate) fn scratch(name: &str) -> PathBuf {
    let id = std::process::id();
    let path = std::env::temp_dir().join(format!("ttymud-{id}-{name}"));
    let _ = fs::remove_dir_all(&path);
    path
}
