use ttymud_text::{Message, ini, sanitize};

use super::save::{Text, allow, checked, problem};
use super::{HostId, Outgoing, Step, World, lines};
use crate::content::{ContentError, File};
use crate::date::stamp;

pub(super) const NEWS: &str = "news.ini";
const NEWS_FILE: &str = "news.txt";
const KEEP: usize = 50;
const SHOWN: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Headline {
    pub(super) when: u64,
    pub(super) text: String,
}

#[derive(Debug, Default)]
pub(super) struct News {
    pub(super) headlines: Vec<Headline>,
    pub(super) dirty: bool,
    pub(super) host: Option<HostId>,
}

impl World {
    pub(super) fn publish(&mut self, text: String) {
        let when = self.now;
        self.send_out(Outgoing::News {
            when,
            text: text.clone(),
        });
        let headlines = &mut self.news.headlines;
        headlines.push(Headline { when, text });
        let old = headlines.len().saturating_sub(KEEP);
        headlines.drain(..old);
        self.news.dirty = true;
        self.post_news();
    }

    pub fn peer_news(&mut self, name: &str, when: u64, text: &str) {
        let text = sanitize(text).trim().to_owned();
        let when = when.min(self.now);
        let Some(peer) = self
            .longline
            .peers
            .iter_mut()
            .find(|peer| peer.name == name)
        else {
            return;
        };
        if !text.is_empty() {
            peer.headlines.push(Headline { when, text });
            let old = peer.headlines.len().saturating_sub(SHOWN);
            peer.headlines.drain(..old);
        }
    }

    pub(super) fn latest_headlines(&self) -> Vec<Headline> {
        let headlines = &self.news.headlines;
        headlines[headlines.len().saturating_sub(SHOWN)..].to_vec()
    }

    pub(super) fn watch_corps(&mut self) {
        for corp in 0..self.corps.len() {
            let mut texts: Vec<String> = self.alert_news(corp).into_iter().collect();
            texts.extend(self.patch_news(corp));
            for text in texts {
                self.publish(text);
            }
        }
    }

    pub(super) fn read_news(&self, args: &[String]) -> Result<Step, Message> {
        match args {
            [] => Ok(lines(self.latest())),
            [word] => {
                let peer = self.find_peer(word)?;
                let shown: Vec<Message> = peer
                    .headlines
                    .iter()
                    .rev()
                    .map(|headline| {
                        Message::game(format!("{} {}", stamp(headline.when), headline.text))
                    })
                    .collect();
                if shown.is_empty() {
                    let name = &peer.name;
                    return Ok(lines(vec![Message::game(format!(
                        "No news from {name} yet."
                    ))]));
                }
                Ok(lines(shown))
            }
            _ => Err(Message::error("Usage: news [city].")),
        }
    }

    pub(super) fn headline(&self) -> String {
        let last = self.news.headlines.last();
        last.map_or_default(|headline| headline.text.clone())
    }

    fn latest(&self) -> Vec<Message> {
        let latest: Vec<Message> = self
            .news
            .headlines
            .iter()
            .rev()
            .take(SHOWN)
            .map(|headline| Message::game(format!("{} {}", stamp(headline.when), headline.text)))
            .collect();
        if latest.is_empty() {
            vec![Message::game("No news yet.")]
        } else {
            latest
        }
    }

    pub(super) fn post_news(&mut self) {
        if let Some(host) = self.news.host {
            let lines = self.latest().iter().map(Message::plain).collect();
            self.post_file(host, File::new(NEWS_FILE, lines));
        }
    }

    pub(super) fn post_file(&mut self, host: HostId, file: File) {
        let files = &mut self.hosts[host.0].files;
        match files.iter_mut().find(|known| known.name == file.name) {
            Some(known) => *known = file,
            None => files.push(file),
        }
    }

    pub(super) fn news_text(&self) -> String {
        let mut text = Text::new("# The news of the city. The server writes this file.");
        text.section("news", &self.city_id);
        for headline in &self.news.headlines {
            text.pair("item", &format!("{} {}", headline.when, headline.text));
        }
        text.finish()
    }

    pub(super) fn restore_news(&mut self, text: &str) -> Result<(), Vec<ContentError>> {
        let mut headlines = parse_news(text)?;
        let old = headlines.len().saturating_sub(KEEP);
        headlines.drain(..old);
        self.news.headlines = headlines;
        self.post_news();
        Ok(())
    }
}

fn parse_news(text: &str) -> Result<Vec<Headline>, Vec<ContentError>> {
    let (sections, mut errors) = ini::read(text);
    let mut headlines = Vec::new();
    for section in &sections {
        if section.kind != "news" {
            let kind = section.kind;
            errors.push(problem(
                section.line,
                format!("unknown section type: {kind}"),
            ));
            continue;
        }
        allow(section, &["item"], &mut errors);
        for pair in section.pairs.iter().filter(|pair| pair.key == "item") {
            let headline = pair.value.split_once(' ').and_then(|(when, text)| {
                Some(Headline {
                    when: when.parse().ok()?,
                    text: text.trim().to_owned(),
                })
            });
            match headline {
                Some(headline) => headlines.push(headline),
                None => errors.push(problem(
                    pair.line,
                    format!("bad item: {} (use a time and a headline)", pair.value),
                )),
            }
        }
    }
    checked(errors, headlines)
}

#[cfg(test)]
mod tests {
    use crate::testing::{enter_job, hired_at, plain, walk, world};
    use crate::{PlayerId, World};

    const NOON: u64 = 20_000 * 86_400 + 12 * 3_600;
    const TIME: u64 = 1_700_000_000;

    fn headlines(world: &World) -> Vec<String> {
        let headlines = world.news.headlines.iter();
        headlines.map(|headline| headline.text.clone()).collect()
    }

    fn acme(world: &World) -> usize {
        let found = world.corps.iter().position(|corp| corp.id == "acme");
        found.unwrap_or_default()
    }

    fn reader(count: usize) -> (World, PlayerId) {
        let mut world = world();
        world.set_time(TIME);
        for number in 1..=count {
            world.publish(format!("Headline {number}."));
        }
        let (player, _) = world.join("ghost");
        (world, player)
    }

    #[test]
    fn the_news_works_anywhere_and_starts_empty() {
        assert_eq!(walk(&["news"]).0, ["No news yet."]);
        assert_eq!(walk(&["go bazaar", "read news.txt"]).0, ["No news yet."]);
    }

    #[test]
    fn news_shows_the_ten_newest_headlines_first_with_their_time() {
        let (mut world, player) = reader(12);
        let shown = plain(&world.command(player, "news"));
        assert_eq!(shown.len(), 10);
        assert_eq!(shown[0], "2023-11-14 22:13 Headline 12.");
        assert_eq!(shown[9], "2023-11-14 22:13 Headline 3.");
        assert_eq!(
            plain(&world.command(player, "news | grep 12")),
            ["2023-11-14 22:13 Headline 12."]
        );
        world.command(player, "go bazaar");
        assert_eq!(plain(&world.command(player, "read news.txt")), shown);
    }

    #[test]
    fn the_news_reports_alerts_and_a_lower_alert_only_when_it_is_calm() {
        let mut world = world();
        world.set_time(NOON);
        let acme = acme(&world);
        world.raise_alert(acme, 30);
        world.tick();
        assert_eq!(headlines(&world), ["Acme Corp raises its alert to yellow."]);
        world.set_time(NOON + 6 * 300);
        world.tick();
        assert_eq!(headlines(&world).len(), 1);
        world.set_time(NOON + 11 * 300);
        world.tick();
        world.tick();
        assert_eq!(
            headlines(&world),
            [
                "Acme Corp raises its alert to yellow.",
                "Acme Corp lowers its alert to green."
            ]
        );
    }

    #[test]
    fn traces_patches_and_new_holes_make_the_news_without_a_handle() {
        let (mut world, player) = hired_at(NOON);
        enter_job(&mut world, player);
        world.raise_trace(player, 100);
        let acme = acme(&world);
        for _ in 0..12 {
            world.expose(acme, "Gatekeep VPN");
        }
        world.set_time(NOON + 2 * 86_400);
        world.tick();
        world.tick();
        assert_eq!(
            headlines(&world),
            [
                "Acme Corp traces an intruder and calls the police.",
                "Acme Corp patches a hole in Gatekeep VPN.",
                "Somebody finds a new hole in Gatekeep VPN at Acme Corp.",
            ]
        );
    }

    #[test]
    fn the_news_keeps_fifty_headlines_and_comes_back_from_news_ini() {
        let (mut world, _) = reader(55);
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "news.ini")
            .map_or_default(|(_, text)| text.clone());
        assert!(
            text.starts_with(
                "# The news of the city. The server writes this file.\n\n[news test]\nitem = 1700000000 Headline 6.\n"
            ),
            "{text}"
        );
        assert!(
            text.ends_with("\nitem = 1700000000 Headline 55.\n"),
            "{text}"
        );
        assert_eq!(world.saves(), Vec::<(String, String)>::new());
        let mut fresh = crate::testing::world();
        fresh.restore("news.ini", &text).unwrap();
        assert_eq!(fresh.news.headlines, world.news.headlines);
        let errors: Vec<String> = fresh
            .restore("news.ini", "[news test]\nitem = soon Hi.\n[old x]\n")
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            errors,
            [
                "line 2: bad item: soon Hi. (use a time and a headline)",
                "line 3: unknown section type: old"
            ]
        );
    }
}
