use super::ChannelDef;
use super::reader::{Name, Reader};
use crate::ini::Section;

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
        self.allow(section, &["desc", "auto"]);
        self.check_name(section.line, id, Name::Plain);
        let desc = self.single(section, "desc").unwrap_or_default().0;
        let auto = match self.optional(section, "auto") {
            None => false,
            Some((word, _)) if word == "yes" => true,
            Some((word, _)) if word == "no" => false,
            Some((word, line)) => {
                self.error(line, format!("bad auto: {word} (use yes or no)"));
                false
            }
        };
        self.channels.push(ChannelDef {
            id: id.to_owned(),
            desc,
            auto,
        });
    }
}
