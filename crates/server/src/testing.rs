use std::fs;
use std::path::PathBuf;

pub(crate) const LAB: &str = "[city lab]\nname = Lab\nstart = hub\n[host hub]\nname = Hub\n\
                              desc = A hub.\n[network net]\nname = Net\n[host gate]\nname = Gate\n\
                              desc = A gate.\nnetwork = net\nroute = hub\n[ice gate/sentry]\n\
                              type = sentry\n";

pub(crate) fn scratch(name: &str) -> PathBuf {
    let id = std::process::id();
    let path = std::env::temp_dir().join(format!("ttymud-{id}-{name}"));
    let _ = fs::remove_dir_all(&path);
    path
}
