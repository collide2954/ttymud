use std::collections::VecDeque;

use ttymud_text::Message;
use ttymud_text::ini::{self, Section};

use super::save::{Text, allow, checked, number, problem};
use super::{Output, PlayerId, Step, World, lines};
use crate::content::ContentError;
use crate::date::stamp;

pub(super) const REPORTS: &str = "reports.ini";
const EVIDENCE: u64 = 15 * 60;
const HISTORY: usize = 10_000;

#[derive(Debug, Default)]
pub(super) struct Reports {
    list: Vec<Report>,
    dirty: bool,
    history: VecDeque<Said>,
}

#[derive(Debug)]
struct Report {
    id: u64,
    when: u64,
    from: String,
    about: String,
    reason: String,
    lines: Vec<String>,
    resolved: Option<(String, u64)>,
}

#[derive(Debug)]
struct Said {
    when: u64,
    speaker: String,
    listener: Option<String>,
    line: String,
}

impl Report {
    fn title(&self) -> String {
        let (id, when) = (self.id, stamp(self.when));
        let head = format!("Report {id}: {when} {} reports {}", self.from, self.about);
        if self.reason.is_empty() {
            format!("{head}.")
        } else {
            format!("{head}: {}", self.reason)
        }
    }
}

impl Reports {
    pub(super) fn save(&mut self) -> Option<(String, String)> {
        std::mem::take(&mut self.dirty).then(|| (REPORTS.to_owned(), self.text()))
    }

    fn text(&self) -> String {
        let mut text = Text::new("# The player reports. The server writes this file.");
        for report in &self.list {
            text.section("report", &report.id.to_string());
            text.pair("when", &report.when.to_string());
            text.pair("from", &report.from);
            text.pair("about", &report.about);
            if !report.reason.is_empty() {
                text.pair("reason", &report.reason);
            }
            for line in &report.lines {
                text.pair("line", line);
            }
            if let Some((name, when)) = &report.resolved {
                text.pair("resolved", &format!("{name} {when}"));
            }
        }
        text.finish()
    }

    pub(super) fn read(&mut self, text: &str) -> Result<(), Vec<ContentError>> {
        let (sections, mut errors) = ini::read(text);
        let mut list = Vec::new();
        for section in &sections {
            if section.kind == "report" {
                list.extend(parse_report(section, &mut errors));
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

fn parse_report(section: &Section<'_>, errors: &mut Vec<ContentError>) -> Option<Report> {
    let Ok(id) = section.id.parse::<u64>() else {
        let id = section.id;
        errors.push(problem(section.line, format!("bad report number: {id}")));
        return None;
    };
    let keys = ["when", "from", "about", "reason", "line", "resolved"];
    allow(section, &keys, errors);
    let values = |key: &'static str| section.pairs.iter().filter(move |pair| pair.key == key);
    for key in ["when", "from", "about"] {
        if values(key).next().is_none() {
            errors.push(problem(section.line, format!("report {id} has no {key}")));
        }
    }
    let text = |key| values(key).next().map_or("", |pair| pair.value).to_owned();
    let when = values("when").next().and_then(|pair| number(pair, errors));
    let mut resolved = None;
    for pair in values("resolved") {
        let parsed = pair.value.rsplit_once(' ').and_then(|(name, when)| {
            let when = when.parse().ok()?;
            Some((name.trim().to_owned(), when))
        });
        if parsed.is_none() {
            let value = pair.value;
            let usage = "use a handle and a time";
            errors.push(problem(
                pair.line,
                format!("bad resolved: {value} ({usage})"),
            ));
        }
        resolved = parsed;
    }
    Some(Report {
        id,
        when: when?,
        from: text("from"),
        about: text("about"),
        reason: text("reason"),
        lines: values("line").map(|pair| pair.value.to_owned()).collect(),
        resolved,
    })
}

impl World {
    pub(super) fn remember(
        &mut self,
        speaker: PlayerId,
        listener: Option<PlayerId>,
        place: &str,
        text: &str,
    ) {
        let Some(state) = self.players.get(&speaker) else {
            return;
        };
        let (key, line) = (state.key(), format!("{place}: <{}> {text}", state.name));
        let listener = listener.and_then(|listener| self.person(listener));
        self.remember_line(key, listener, line);
    }

    pub(super) fn remember_line(
        &mut self,
        speaker: String,
        listener: Option<String>,
        line: String,
    ) {
        let said = Said {
            when: self.now,
            speaker,
            listener,
            line,
        };
        let now = self.now;
        let history = &mut self.moderation.reports.history;
        history.push_back(said);
        while history.len() > HISTORY
            || history
                .front()
                .is_some_and(|said| now.saturating_sub(said.when) > EVIDENCE)
        {
            history.pop_front();
        }
    }

    pub(super) fn report(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let [word, reason @ ..] = args else {
            return Err(Message::error("Usage: report <handle> [reason]."));
        };
        let (key, about) = self.find_person(word)?;
        let (Some(from_key), Some(from)) = (self.person(player), self.handle(player)) else {
            return Ok(Step::default());
        };
        if from_key == key {
            return Err(Message::error("You cannot report yourself."));
        }
        let reports = &self.moderation.reports;
        let open = |report: &&Report| report.resolved.is_none();
        if reports
            .list
            .iter()
            .filter(open)
            .any(|report| report.from == from && report.about == about)
        {
            return Err(Message::error(format!(
                "You reported {about} before. That report is still open."
            )));
        }
        let now = self.now;
        let lines = reports
            .history
            .iter()
            .filter(|said| said.speaker == key && now.saturating_sub(said.when) <= EVIDENCE)
            .filter(|said| {
                said.listener
                    .as_ref()
                    .is_none_or(|heard| *heard == from_key)
            })
            .map(|said| format!("{} {}", stamp(said.when), said.line))
            .collect();
        let id = reports
            .list
            .iter()
            .map(|report| report.id)
            .max()
            .unwrap_or(0)
            + 1;
        let report = Report {
            id,
            when: now,
            from: from.to_owned(),
            about: about.clone(),
            reason: reason.join(" "),
            lines,
            resolved: None,
        };
        let news = format!("New report {id}: {} reports {about}.", report.from);
        self.moderation.reports.list.push(report);
        self.moderation.reports.dirty = true;
        self.tell_moderators(&format!("{news} Type reports {id} to see it."));
        Ok(Step::show(vec![Message::game(format!(
            "You report {about}. The moderators get your report and the chat of {about} from the last 15 minutes."
        ))]))
    }

    fn tell_moderators(&mut self, text: &str) {
        let moderators: Vec<PlayerId> = self
            .players
            .keys()
            .copied()
            .filter(|&player| self.is_moderator(player))
            .collect();
        for player in moderators {
            let message = Message::game(text);
            self.outbox.push(Output { player, message });
        }
    }

    pub(super) fn reports(&self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.moderator_only(player, "reports")?;
        let list = &self.moderation.reports.list;
        let id = match args {
            [] => {
                let open = list.iter().filter(|report| report.resolved.is_none());
                let mut shown: Vec<Message> =
                    open.map(|report| Message::game(report.title())).collect();
                if shown.is_empty() {
                    return Ok(lines(vec![Message::game("There are no open reports.")]));
                }
                shown.push(Message::game(
                    "Type reports <number> to see a report. Type resolve <number> to close it.",
                ));
                return Ok(lines(shown));
            }
            [word] => word.parse::<u64>().ok(),
            _ => None,
        };
        let id = id.ok_or_else(|| Message::error("Usage: reports [number]."))?;
        let report = list
            .iter()
            .find(|report| report.id == id)
            .ok_or_else(|| Message::error(format!("No report {id}.")))?;
        let mut shown = vec![Message::game(report.title())];
        if report.lines.is_empty() {
            let about = &report.about;
            shown.push(Message::game(format!(
                "No chat from {about} in the 15 minutes before the report."
            )));
        }
        shown.extend(report.lines.iter().map(|line| Message::game(line.as_str())));
        if let Some((name, when)) = &report.resolved {
            let when = stamp(*when);
            shown.push(Message::game(format!(
                "{name} resolved this report at {when}."
            )));
        }
        Ok(lines(shown))
    }

    pub(super) fn resolve(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.moderator_only(player, "resolve")?;
        let id = match args {
            [word] => word.parse::<u64>().ok(),
            _ => None,
        };
        let id = id.ok_or_else(|| Message::error("Usage: resolve <number>."))?;
        let name = self.handle(player).unwrap_or_default().to_owned();
        let now = self.now;
        let reports = &mut self.moderation.reports;
        let report = reports
            .list
            .iter_mut()
            .find(|report| report.id == id)
            .ok_or_else(|| Message::error(format!("No report {id}.")))?;
        if report.resolved.is_some() {
            return Err(Message::error(format!("Report {id} is already resolved.")));
        }
        report.resolved = Some((name, now));
        let about = report.about.clone();
        reports.dirty = true;
        self.audit(player, &format!("resolves report {id} about {about}."));
        Ok(Step::show(vec![Message::game(format!(
            "You resolve report {id}."
        ))]))
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{hacker, heard, plain, world};
    use crate::{PlayerId, World};

    const MODERATORS: &str = "[moderator a]\n";
    const HINT: &str = "Type reports <number> to see a report. Type resolve <number> to close it.";

    fn three() -> (World, PlayerId, PlayerId, PlayerId) {
        let mut world = world();
        world.restore("moderators.ini", MODERATORS).unwrap();
        world.set_time(60);
        let nyx = hacker(&mut world, "a", "nyx");
        let zer0 = hacker(&mut world, "b", "zer0");
        let kay = hacker(&mut world, "c", "kay");
        (world, nyx, zer0, kay)
    }

    #[test]
    fn a_report_keeps_the_recent_chat_of_the_handle() {
        let (mut world, nyx, zer0, kay) = three();
        world.command(zer0, "say old news");
        world.set_time(200);
        world.command(zer0, "#city buy cheap creds");
        world.set_time(700);
        world.command(zer0, "tell kay hi there");
        world.command(zer0, "tell nyx psst");
        world.command(kay, "say hello");
        world.set_time(1_000);
        let outputs = world.command(kay, "report zer0 spam in city");
        assert_eq!(
            heard(&outputs, kay),
            [
                "You report zer0. The moderators get your report and the chat of zer0 from the last 15 minutes."
            ]
        );
        assert_eq!(
            heard(&outputs, nyx),
            ["New report 1: kay reports zer0. Type reports 1 to see it."]
        );
        assert_eq!(heard(&outputs, zer0), Vec::<String>::new());
        assert_eq!(
            plain(&world.command(nyx, "reports 1")),
            [
                "Report 1: 1970-01-01 00:16 kay reports zer0: spam in city",
                "1970-01-01 00:03 #city: <zer0> buy cheap creds",
                "1970-01-01 00:11 tell: <zer0> hi there",
            ]
        );
    }

    #[test]
    fn moderators_list_and_resolve_reports() {
        let (mut world, nyx, zer0, kay) = three();
        world.command(kay, "report zer0");
        world.command(zer0, "report kay rude");
        let mut said = |line: &str| plain(&world.command(nyx, line));
        assert_eq!(
            said("reports"),
            [
                "Report 1: 1970-01-01 00:01 kay reports zer0.",
                "Report 2: 1970-01-01 00:01 zer0 reports kay: rude",
                HINT,
            ]
        );
        assert_eq!(
            said("reports 1")[1],
            "No chat from zer0 in the 15 minutes before the report."
        );
        assert_eq!(said("resolve 1"), ["You resolve report 1."]);
        assert_eq!(said("resolve 1"), ["Report 1 is already resolved."]);
        assert_eq!(said("resolve 9"), ["No report 9."]);
        assert_eq!(said("resolve x"), ["Usage: resolve <number>."]);
        assert_eq!(said("reports x"), ["Usage: reports [number]."]);
        assert_eq!(
            said("reports"),
            ["Report 2: 1970-01-01 00:01 zer0 reports kay: rude", HINT]
        );
        assert_eq!(
            said("reports 1").last().map(String::as_str),
            Some("nyx resolved this report at 1970-01-01 00:01.")
        );
        said("resolve 2");
        assert_eq!(said("reports"), ["There are no open reports."]);
    }

    #[test]
    fn a_report_needs_another_handle_and_one_open_report_at_a_time() {
        let (mut world, _, zer0, kay) = three();
        let mut said = |line: &str| plain(&world.command(kay, line));
        assert_eq!(said("report"), ["Usage: report <handle> [reason]."]);
        assert_eq!(said("report kay"), ["You cannot report yourself."]);
        assert_eq!(said("report nobody"), ["No handle named nobody."]);
        said("report zer0");
        assert_eq!(
            said("report zer0 again"),
            ["You reported zer0 before. That report is still open."]
        );
        assert_eq!(
            plain(&world.command(zer0, "reports")),
            ["Only moderators can use reports."]
        );
    }

    #[test]
    fn reports_come_back_from_their_save() {
        let (mut world, nyx, zer0, kay) = three();
        world.command(zer0, "#city buy = cheap # creds");
        world.command(kay, "report zer0 spam");
        world.command(nyx, "resolve 1");
        world.command(kay, "report nyx abuse");
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "reports.ini")
            .map(|(_, text)| text.as_str());
        let expected = [
            "# The player reports. The server writes this file.",
            "",
            "[report 1]",
            "when = 60",
            "from = kay",
            "about = zer0",
            "reason = spam",
            "line = 1970-01-01 00:01 #city: <zer0> buy = cheap # creds",
            "resolved = nyx 60",
            "",
            "[report 2]",
            "when = 60",
            "from = kay",
            "about = nyx",
            "reason = abuse",
            "",
        ]
        .join("\n");
        assert_eq!(text, Some(expected.as_str()));
        let mut fresh = crate::testing::world();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        fresh.restore("moderators.ini", MODERATORS).unwrap();
        let (nyx, _) = fresh.connect("a");
        assert_eq!(
            plain(&fresh.command(nyx, "reports")),
            ["Report 2: 1970-01-01 00:01 kay reports nyx: abuse", HINT]
        );
        assert_eq!(
            plain(&fresh.command(nyx, "reports 1"))[1],
            "1970-01-01 00:01 #city: <zer0> buy = cheap # creds"
        );
        let (kay, _) = fresh.connect("c");
        let outputs = fresh.command(kay, "report zer0");
        assert_eq!(
            heard(&outputs, nyx),
            ["New report 3: kay reports zer0. Type reports 3 to see it."]
        );
    }

    #[test]
    fn a_bad_report_file_fails() {
        let mut world = world();
        let errors: Vec<String> = world
            .restore(
                "reports.ini",
                "[report x]\n[report 2]\nwhen = soon\ncolor = red\nresolved = nyx\n",
            )
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            errors,
            [
                "line 1: bad report number: x",
                "line 2: report 2 has no from",
                "line 2: report 2 has no about",
                "line 3: bad number: soon",
                "line 4: unknown key in report 2: color",
                "line 5: bad resolved: nyx (use a handle and a time)",
            ]
        );
    }
}
