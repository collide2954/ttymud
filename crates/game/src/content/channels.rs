use ttymud_text::ini::Section;

use super::ChannelDef;
use super::reader::{Name, Reader};

impl Reader {
    pub(super) fn channel(&mut self, section: &Section<'_>) {
        let id = section.id;
        if self.channels.iter().any(|channel| channel.id == id) {
            self.error(
                section.line,
                format!("channel {id} is defined more than once"),
            );
            return;
        }
        self.allow(section, &["desc", "auto", "linked"]);
        self.check_name(section.line, id, Name::Plain);
        if id == "crew" {
            self.error(
                section.line,
                "the channel name crew is for crews".to_owned(),
            );
        }
        let desc = self.single(section, "desc").unwrap_or_default().0;
        let auto = self.yes_or_no(section, "auto");
        let linked = self.yes_or_no(section, "linked");
        let other = self.channels.iter().find(|channel| channel.linked);
        if let Some(other) = other.filter(|_| linked) {
            let message = format!(
                "channel {id} cannot be linked, because channel {} is linked",
                other.id
            );
            self.error(section.line, message);
        }
        self.channels.push(ChannelDef {
            id: id.to_owned(),
            desc,
            auto,
            linked,
        });
    }

    fn yes_or_no(&mut self, section: &Section<'_>, key: &str) -> bool {
        match self.optional(section, key) {
            None => false,
            Some((word, _)) if word == "yes" => true,
            Some((word, _)) if word == "no" => false,
            Some((word, line)) => {
                self.error(line, format!("bad {key}: {word} (use yes or no)"));
                false
            }
        }
    }
}
