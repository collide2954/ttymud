use super::World;
use crate::content::File;

const BURN_LIST: &str = "burn-list.txt";

impl World {
    pub(super) fn post_burns(&mut self) {
        let Some(host) = self.burns_host else {
            return;
        };
        let file = File {
            name: BURN_LIST.to_owned(),
            lines: vec!["Nobody is on the burn list yet.".to_owned()],
        };
        let files = &mut self.hosts[host.0].files;
        files.retain(|known| known.name != BURN_LIST);
        files.push(file);
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::walk;

    #[test]
    fn the_burn_list_is_a_file_on_its_host() {
        assert_eq!(
            walk(&["go bazaar", "read burn-list.txt"]).0,
            ["Nobody is on the burn list yet."]
        );
    }
}
