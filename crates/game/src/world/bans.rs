use std::net::IpAddr;

use ttymud_text::Message;
use ttymud_text::ini::{self, Section};

use super::save::{Text, allow, checked, number, problem};
use super::{PlayerId, Step, World, lines};
use crate::content::ContentError;
use crate::date::stamp;

pub(super) const BANS: &str = "bans.ini";
pub const BANNED: &str = "You are banned from this server.";

#[derive(Debug, Default)]
pub(super) struct Bans {
    list: Vec<Ban>,
    dirty: bool,
}

#[derive(Debug)]
struct Ban {
    account: String,
    handle: String,
    by: String,
    when: u64,
    address: Option<IpAddr>,
}

impl Bans {
    pub(super) fn save(&mut self) -> Option<(String, String)> {
        std::mem::take(&mut self.dirty).then(|| (BANS.to_owned(), self.text()))
    }

    fn text(&self) -> String {
        let mut text = Text::new("# The bans of this server. The server writes this file.");
        for ban in &self.list {
            text.section("ban", &ban.account);
            text.pair("handle", &ban.handle);
            text.pair("by", &ban.by);
            text.pair("when", &ban.when.to_string());
            if let Some(address) = ban.address {
                text.pair("address", &address.to_string());
            }
        }
        text.finish()
    }

    pub(super) fn read(&mut self, text: &str) -> Result<(), Vec<ContentError>> {
        let (sections, mut errors) = ini::read(text);
        let mut list = Vec::new();
        for section in &sections {
            if section.kind == "ban" {
                list.extend(parse_ban(section, &mut errors));
            } else {
                let kind = section.kind;
                errors.push(problem(
                    section.line,
                    format!("unknown section type: {kind}"),
                ));
            }
        }
        self.list = checked(errors, list)?;
        Ok(())
    }
}

fn parse_ban(section: &Section<'_>, errors: &mut Vec<ContentError>) -> Option<Ban> {
    allow(section, &["handle", "by", "when", "address"], errors);
    let id = section.id;
    let value = |key: &str| section.pairs.iter().find(|pair| pair.key == key);
    for key in ["handle", "by", "when"] {
        if value(key).is_none() {
            errors.push(problem(section.line, format!("ban {id} has no {key}")));
        }
    }
    let when = value("when").and_then(|pair| number(pair, errors));
    let address = value("address").and_then(|pair| {
        let address = pair.value.parse().ok();
        if address.is_none() {
            let value = pair.value;
            errors.push(problem(pair.line, format!("bad address: {value}")));
        }
        address
    });
    Some(Ban {
        account: id.to_owned(),
        handle: value("handle")?.value.to_owned(),
        by: value("by")?.value.to_owned(),
        when: when?,
        address,
    })
}

impl World {
    #[must_use]
    pub fn banned(&self, account: &str, address: Option<IpAddr>) -> bool {
        let list = &self.moderation.bans.list;
        list.iter()
            .any(|ban| ban.account == account || (address.is_some() && ban.address == address))
    }

    pub fn set_address(&mut self, player: PlayerId, address: IpAddr) {
        if let Some(state) = self.players.get_mut(&player) {
            state.address = Some(address);
        }
    }

    pub(super) fn ban(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.moderator_only(player, "ban")?;
        let (word, with_address) = match args {
            [word] => (word, false),
            [word, ip] if ip.eq_ignore_ascii_case("ip") => (word, true),
            _ => return Err(Message::error("Usage: ban <handle> [ip].")),
        };
        let (key, name) = self.subject(word, "ban")?;
        if self
            .moderation
            .bans
            .list
            .iter()
            .any(|ban| ban.account == key)
        {
            return Err(Message::error(format!("{name} is already banned.")));
        }
        let address = if with_address {
            let online = self.players_of(&key).into_iter();
            let mut found = online.filter_map(|id| self.players.get(&id)?.address);
            let unknown = format!(
                "{name} is not online, so the IP address is not known. Type ban {name} to ban only the account."
            );
            Some(found.next().ok_or_else(|| Message::error(unknown))?)
        } else {
            None
        };
        let ban = Ban {
            account: key.clone(),
            handle: name.clone(),
            by: self.handle(player).unwrap_or_default().to_owned(),
            when: self.now,
            address,
        };
        self.moderation.bans.list.push(ban);
        self.moderation.bans.dirty = true;
        self.disconnect(&key, BANNED);
        let what = match address {
            Some(_) => format!("{name} and the IP address of {name}"),
            None => name,
        };
        self.audit(player, &format!("bans {what}."));
        Ok(Step::show(vec![Message::game(format!("You ban {what}."))]))
    }

    pub(super) fn unban(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.moderator_only(player, "unban")?;
        let [word] = args else {
            return Err(Message::error("Usage: unban <handle>."));
        };
        let bans = &mut self.moderation.bans;
        let found = bans
            .list
            .iter()
            .position(|ban| ban.handle.eq_ignore_ascii_case(word));
        let Some(index) = found else {
            return Err(Message::error(format!("{word} is not banned.")));
        };
        let name = bans.list.remove(index).handle;
        bans.dirty = true;
        self.audit(player, &format!("unbans {name}."));
        Ok(Step::show(vec![Message::game(format!(
            "You unban {name}."
        ))]))
    }

    pub(super) fn bans(&self, player: PlayerId) -> Result<Step, Message> {
        self.moderator_only(player, "bans")?;
        let list = &self.moderation.bans.list;
        if list.is_empty() {
            return Ok(lines(vec![Message::game("Nobody is banned.")]));
        }
        let shown = list.iter().map(|ban| {
            let when = stamp(ban.when);
            let tail = if ban.address.is_some() {
                ", with the IP address"
            } else {
                ""
            };
            let (name, by) = (&ban.handle, &ban.by);
            Message::game(format!("{name}: banned by {by} at {when}{tail}."))
        });
        Ok(lines(shown.collect()))
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use crate::testing::{hacker, plain, world};
    use crate::{PlayerId, World};

    const MODERATORS: &str = "[moderator a]\n";
    const BANNED: &str = "You are banned from this server.";

    fn moderated() -> (World, PlayerId, PlayerId, IpAddr) {
        let mut world = world();
        world.restore("moderators.ini", MODERATORS).unwrap();
        world.set_time(60);
        let nyx = hacker(&mut world, "a", "nyx");
        let zer0 = hacker(&mut world, "b", "zer0");
        let address = IpAddr::from([203, 0, 113, 5]);
        world.set_address(zer0, address);
        (world, nyx, zer0, address)
    }

    #[test]
    fn a_ban_drops_the_handle_and_refuses_the_account() {
        let (mut world, nyx, zer0, address) = moderated();
        assert_eq!(plain(&world.command(nyx, "ban zer0")), ["You ban zer0."]);
        assert_eq!(world.drops(), [(zer0, BANNED.to_owned())]);
        assert!(world.banned("b", None));
        assert!(!world.banned("c", Some(address)));
        let mut said = |line: &str| plain(&world.command(nyx, line));
        assert_eq!(said("ban ZER0"), ["zer0 is already banned."]);
        assert_eq!(said("unban zer0"), ["You unban zer0."]);
        assert_eq!(said("unban zer0"), ["zer0 is not banned."]);
        assert!(!world.banned("b", None));
    }

    #[test]
    fn an_ip_ban_also_refuses_the_address() {
        let (mut world, nyx, _, address) = moderated();
        assert_eq!(
            plain(&world.command(nyx, "ban zer0 ip")),
            ["You ban zer0 and the IP address of zer0."]
        );
        assert!(world.banned("c", Some(address)));
        assert!(!world.banned("c", Some(IpAddr::from([203, 0, 113, 6]))));
        world.command(nyx, "unban zer0");
        assert!(!world.banned("c", Some(address)));
    }

    #[test]
    fn a_ban_checks_its_handle() {
        let (mut world, nyx, zer0, _) = moderated();
        assert_eq!(
            plain(&world.command(zer0, "ban nyx")),
            ["Only moderators can use ban."]
        );
        world.leave(zer0);
        let mut said = |line: &str| plain(&world.command(nyx, line));
        assert_eq!(
            said("ban zer0 ip"),
            [
                "zer0 is not online, so the IP address is not known. Type ban zer0 to ban only the account."
            ]
        );
        assert_eq!(said("ban nyx"), ["You cannot ban a moderator."]);
        assert_eq!(said("ban"), ["Usage: ban <handle> [ip]."]);
        assert_eq!(said("ban zer0 now"), ["Usage: ban <handle> [ip]."]);
        assert_eq!(said("bans"), ["Nobody is banned."]);
        assert_eq!(said("ban zer0"), ["You ban zer0."]);
    }

    #[test]
    fn bans_are_listed_saved_and_audited() {
        let (mut world, nyx, _, address) = moderated();
        world.command(nyx, "ban zer0 ip");
        assert_eq!(
            plain(&world.command(nyx, "bans")),
            ["zer0: banned by nyx at 1970-01-01 00:01, with the IP address."]
        );
        let saves = world.saves();
        let file = |path: &str| {
            saves
                .iter()
                .find(|(name, _)| name == path)
                .map(|(_, text)| text.clone())
        };
        let expected = "# The bans of this server. The server writes this file.\n\n\
                        [ban b]\nhandle = zer0\nby = nyx\nwhen = 60\naddress = 203.0.113.5\n";
        assert_eq!(file("bans.ini").as_deref(), Some(expected));
        let audit = file("audit.log").unwrap_or_default();
        assert_eq!(
            audit.lines().last(),
            Some("1970-01-01 00:01 nyx bans zer0 and the IP address of zer0.")
        );
        let mut fresh = crate::testing::world();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        assert!(fresh.banned("b", None));
        assert!(fresh.banned("c", Some(address)));
    }

    #[test]
    fn a_bad_ban_file_fails() {
        let mut world = world();
        let errors: Vec<String> = world
            .restore(
                "bans.ini",
                "[ban b]\nhandle = zer0\nwhen = soon\naddress = here\ncolor = red\n",
            )
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            errors,
            [
                "line 1: ban b has no by",
                "line 3: bad number: soon",
                "line 4: bad address: here",
                "line 5: unknown key in ban b: color",
            ]
        );
    }
}
