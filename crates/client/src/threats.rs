use std::time::{Duration, Instant};

use ttymud_text::{Kind, Message, Style};

const TICK: Duration = Duration::from_secs(2);
const GRACE: Duration = TICK;
const READ_TICKS: u32 = 2;
const MOST: usize = 16;
const UNKNOWN: &str = "Unknown ICE";
const READS: &str = "reads";
const CUTS: &str = "cuts";

#[derive(Debug)]
pub(crate) struct Threat {
    who: String,
    act: String,
    pub(crate) label: Message,
    due: Instant,
}

impl Threat {
    pub(crate) fn left(&self, now: Instant) -> Option<u128> {
        let millis = self.due.saturating_duration_since(now).as_millis();
        (millis > 0).then(|| millis.div_ceil(1_000))
    }

    fn change(&self, now: Instant) -> Instant {
        let millis = self.due.saturating_duration_since(now).as_millis();
        if millis == 0 {
            return self.gone();
        }
        let step = (millis - 1) % 1_000 + 1;
        now + Duration::from_millis(u64::try_from(step).unwrap_or(1_000))
    }

    fn gone(&self) -> Instant {
        self.due.checked_add(GRACE).unwrap_or(self.due)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Threats(Vec<Threat>);

impl Threats {
    pub(crate) fn hear(&mut self, message: &Message, now: Instant) {
        let text = message.plain();
        match message.kind {
            Kind::Telegraph if text.contains("[SOC] Analyst ") => self.soc(message, &text, now),
            Kind::Telegraph => self.ice(message, &text, now),
            Kind::Game if text.starts_with("crack breaks ") => {
                if let Some(name) = styled(message, Style::Ice).next() {
                    self.0.retain(|threat| threat.who != name);
                }
            }
            Kind::Game | Kind::Error | Kind::Chat => {}
        }
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn live(&mut self, now: Instant) -> &[Threat] {
        self.0.retain(|threat| now < threat.gone());
        &self.0
    }

    pub(crate) fn wake(&self, now: Instant) -> Option<Instant> {
        self.0.iter().map(|threat| threat.change(now)).min()
    }

    fn ice(&mut self, message: &Message, text: &str, now: Instant) {
        if styled(message, Style::Player).next().is_some() {
            return;
        }
        let name = styled(message, Style::Ice).next();
        let who = name.unwrap_or(UNKNOWN);
        let action = styled(message, Style::Danger).next();
        if text.contains(" starts to compile ") {
            let (Some(action), Some(ticks)) = (action, countdown(text)) else {
                return;
            };
            let style = if name.is_some() {
                Style::Ice
            } else {
                Style::Plain
            };
            let label = Message::new(Kind::Telegraph)
                .span(style, who)
                .text(" ")
                .span(Style::Danger, action);
            self.add(who, action, label, ticks, now);
        } else if text.contains(" fires ") || text.contains(" sends an ") {
            self.0.retain(|threat| {
                Some(threat.act.as_str()) != action || (threat.who != who && threat.who != UNKNOWN)
            });
        } else if text.contains(" loses you") {
            self.0.retain(|threat| threat.who != who);
        }
    }

    fn soc(&mut self, message: &Message, text: &str, now: Instant) {
        let Some(analyst) = styled(message, Style::Player).next() else {
            return;
        };
        let hosts: Vec<&str> = styled(message, Style::Host).collect();
        let label = |act: &str| {
            Message::new(Kind::Telegraph)
                .span(Style::Player, analyst)
                .text(format!(" {act} "))
        };
        match hosts.as_slice() {
            [host] if text.contains(" turns to the logs of ") => {
                let ticks = countdown(text).unwrap_or(READ_TICKS);
                let label = label(READS).span(Style::Host, *host);
                self.add(analyst, READS, label, ticks, now);
            }
            [from, to] if text.contains(" starts to cut ") => {
                let Some(ticks) = countdown(text) else {
                    return;
                };
                let label = label(CUTS)
                    .span(Style::Host, *from)
                    .text(" to ")
                    .span(Style::Host, *to);
                self.add(analyst, CUTS, label, ticks, now);
            }
            _ if text.contains(" reads the logs of ") => self.end(analyst, READS),
            _ if text.contains(" cuts the route from ") => self.end(analyst, CUTS),
            _ => {}
        }
    }

    fn end(&mut self, who: &str, act: &str) {
        self.0
            .retain(|threat| threat.who != who || threat.act != act);
    }

    fn add(&mut self, who: &str, act: &str, label: Message, ticks: u32, now: Instant) {
        let Some(due) = TICK
            .checked_mul(ticks)
            .and_then(|wait| now.checked_add(wait))
        else {
            return;
        };
        self.end(who, act);
        self.0.push(Threat {
            who: who.to_owned(),
            act: act.to_owned(),
            label,
            due,
        });
        self.0.sort_by_key(|threat| threat.due);
        self.0.truncate(MOST);
    }
}

fn styled(message: &Message, style: Style) -> impl Iterator<Item = &str> {
    message
        .spans
        .iter()
        .filter(move |span| span.style == style)
        .map(|span| span.text.as_str())
}

fn countdown(text: &str) -> Option<u32> {
    let (_, inside) = text.strip_suffix(").")?.rsplit_once('(')?;
    let count = inside
        .strip_suffix(" ticks")
        .or_else(|| inside.strip_suffix(" tick"))?;
    count.parse().ok()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use ttymud_text::{Kind, Message, Style};

    use super::Threats;

    fn ice(name: &str) -> Message {
        Message::new(Kind::Telegraph).span(Style::Ice, name)
    }

    fn compiles(ice: Message, action: &str, ticks: u32) -> Message {
        ice.text(" starts to compile ")
            .span(Style::Danger, action)
            .text(format!(" ({ticks} ticks)."))
    }

    fn compile(name: &str, action: &str, ticks: u32) -> Message {
        compiles(ice(name), action, ticks)
    }

    fn analyst(name: &str) -> Message {
        Message::new(Kind::Telegraph)
            .text("[SOC] Analyst ")
            .span(Style::Player, name)
    }

    fn heard(threats: &mut Threats, now: Instant) -> Vec<(String, Option<u128>)> {
        let live = threats.live(now);
        live.iter()
            .map(|threat| (threat.label.plain(), threat.left(now)))
            .collect()
    }

    fn after(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    #[test]
    fn an_ice_telegraph_counts_down_the_seconds_to_its_action() {
        let (start, mut threats) = (Instant::now(), Threats::default());
        threats.hear(&compile("sentry", "ALERT", 3), start);
        let sentry = |left| vec![("sentry ALERT".to_owned(), left)];
        assert_eq!(heard(&mut threats, start), sentry(Some(6)));
        assert_eq!(heard(&mut threats, after(start, 500)), sentry(Some(6)));
        assert_eq!(heard(&mut threats, after(start, 5_000)), sentry(Some(1)));
        assert_eq!(heard(&mut threats, after(start, 6_000)), sentry(None));
        assert_eq!(heard(&mut threats, after(start, 8_000)), []);
    }

    #[test]
    fn the_action_a_lost_signal_or_a_crack_ends_the_threat() {
        let (now, mut threats) = (Instant::now(), Threats::default());
        let ends = [
            ice("sentry")
                .text(" sends an ")
                .span(Style::Danger, "ALERT")
                .text("."),
            ice("sentry").text(" loses you."),
            ice("sentry").text(" loses your signal."),
            Message::game("crack breaks ")
                .span(Style::Ice, "sentry")
                .text("."),
        ];
        for end in ends {
            threats.hear(&compile("sentry", "ALERT", 3), now);
            threats.hear(&end, now);
            assert_eq!(heard(&mut threats, now), [], "{}", end.plain());
        }
        let unknown = Message::new(Kind::Telegraph).text("Unknown ICE");
        threats.hear(&compiles(unknown, "TAR", 2), now);
        assert_eq!(
            heard(&mut threats, now),
            [("Unknown ICE TAR".to_owned(), Some(4))]
        );
        let fires = ice("tarpit").text(" fires ").span(Style::Danger, "TAR");
        threats.hear(&fires.text(". Cycles: 4 -> 0."), now);
        assert_eq!(heard(&mut threats, now), []);
    }

    #[test]
    fn a_telegraph_against_a_crewmate_is_not_a_threat_to_you() {
        let (now, mut threats) = (Instant::now(), Threats::default());
        let against = ice("sentry")
            .text(" starts to compile ")
            .span(Style::Danger, "ALERT")
            .text(" against ")
            .span(Style::Player, "nyx")
            .text(" (3 ticks).");
        threats.hear(&against, now);
        assert_eq!(heard(&mut threats, now), []);
    }

    #[test]
    fn the_reads_and_the_cuts_of_the_soc_count_down_too() {
        let (now, mut threats) = (Instant::now(), Threats::default());
        let turns = analyst("h.reyes")
            .text(" turns to the logs of ")
            .span(Style::Host, "vpn");
        threats.hear(&turns.clone().text(" (1 tick)."), now);
        assert_eq!(
            heard(&mut threats, now),
            [("h.reyes reads vpn".to_owned(), Some(2))]
        );
        let reads = analyst("h.reyes")
            .text(" reads the logs of ")
            .span(Style::Host, "vpn")
            .text(" and finds nothing.");
        threats.hear(&reads, now);
        assert_eq!(heard(&mut threats, now), []);
        threats.hear(&turns.text("."), now);
        assert_eq!(
            heard(&mut threats, now),
            [("h.reyes reads vpn".to_owned(), Some(4))],
            "an older server does not send the ticks"
        );
        let route = |message: Message| {
            message
                .span(Style::Host, "vpn")
                .text(" to ")
                .span(Style::Host, "core")
        };
        threats.hear(&reads, now);
        let starts = route(analyst("h.reyes").text(" starts to cut the route from "));
        threats.hear(&starts.text(" (2 ticks)."), now);
        assert_eq!(
            heard(&mut threats, now),
            [("h.reyes cuts vpn to core".to_owned(), Some(4))]
        );
        let cuts = route(analyst("h.reyes").text(" cuts the route from "));
        threats.hear(&cuts.text("."), now);
        assert_eq!(heard(&mut threats, now), []);
    }

    #[test]
    fn a_new_telegraph_takes_the_place_of_the_old_one_and_the_soonest_comes_first() {
        let (now, mut threats) = (Instant::now(), Threats::default());
        threats.hear(&compile("sentry", "ALERT", 5), now);
        threats.hear(&compile("scorch", "BURN", 2), now);
        threats.hear(&compile("sentry", "ALERT", 3), now);
        assert_eq!(
            heard(&mut threats, now),
            [
                ("scorch BURN".to_owned(), Some(4)),
                ("sentry ALERT".to_owned(), Some(6))
            ]
        );
        let mut alert = Message::new(Kind::Telegraph).text("Alert: ");
        alert.spans.extend(compile("hound", "TRACE BURST", 4).spans);
        threats.hear(&alert, now);
        assert_eq!(heard(&mut threats, now).len(), 3, "verbose adds Alert:");
        threats.clear();
        assert_eq!(heard(&mut threats, now), []);
    }

    #[test]
    fn the_next_redraw_comes_when_a_number_changes() {
        let (start, mut threats) = (Instant::now(), Threats::default());
        assert_eq!(threats.wake(start), None);
        threats.hear(&compile("sentry", "ALERT", 3), start);
        assert_eq!(threats.wake(start), Some(after(start, 1_000)));
        assert_eq!(threats.wake(after(start, 1_500)), Some(after(start, 2_000)));
        assert_eq!(threats.wake(after(start, 6_000)), Some(after(start, 8_000)));
    }

    #[test]
    fn a_server_cannot_flood_the_threats_or_push_them_out_of_time() {
        let (now, mut threats) = (Instant::now(), Threats::default());
        for index in 0..100 {
            threats.hear(&compile(&format!("ice{index}"), "BURN", 2), now);
        }
        threats.hear(&compile("forever", "BURN", u32::MAX), now);
        assert_eq!(heard(&mut threats, now).len(), 16);
    }
}
