use std::io;

use ttymud_text::{Kind, Message, Status};

use crate::Shown;
use crate::editor::Key;
use crate::session::{Typed, View};

pub(crate) struct Narrated<V> {
    view: V,
    last: Option<Status>,
}

impl<V> Narrated<V> {
    pub(crate) fn new(view: V) -> Self {
        Self { view, last: None }
    }
}

impl<V: View> View for Narrated<V> {
    fn show(&mut self, shown: Shown) -> io::Result<()> {
        match shown {
            Shown::Status(status) => {
                if let Some(last) = &self.last {
                    for line in changes(last, &status) {
                        self.view.show(Shown::Line(Message::game(line)))?;
                    }
                }
                self.last = Some(status.clone());
                self.view.show(Shown::Status(status))
            }
            Shown::Line(message) if message.kind == Kind::Telegraph => {
                let mut alert = Message::new(Kind::Telegraph).text("Alert: ");
                alert.spans.extend(message.spans);
                self.view.show(Shown::Line(alert))
            }
            other => self.view.show(other),
        }
    }

    fn press(&mut self, key: Key) -> io::Result<Typed> {
        self.view.press(key)
    }

    fn resize(&mut self, width: u16, height: u16) -> io::Result<()> {
        self.view.resize(width, height)
    }

    fn submitted(&mut self) {
        self.view.submitted();
    }

    fn finish(&mut self) -> io::Result<()> {
        self.view.finish()
    }
}

fn changes(last: &Status, next: &Status) -> Vec<String> {
    let mut lines = Vec::new();
    match (&last.run, &next.run) {
        (None, Some(run)) => lines.push(format!("You are now in {}.", run.network)),
        (Some(run), None) => lines.push(format!("You left {}.", run.network)),
        (Some(before), Some(after)) => {
            if before.trace != after.trace {
                lines.push(format!("Trace is now {} percent.", after.trace));
            }
            if before.access != after.access {
                lines.push(format!("Access is now {}.", after.access));
            }
        }
        (None, None) => {}
    }
    if last.integrity != next.integrity {
        lines.push(format!("Integrity is now {}.", next.integrity));
    }
    if last.heat != next.heat {
        lines.push(format!("Heat is now {} of {}.", next.heat, next.heat_max));
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::io;

    use ttymud_text::{Kind, Message, RunStatus, Status, Style};

    use super::Narrated;
    use crate::Shown;
    use crate::session::View;

    #[derive(Default)]
    struct Record(Vec<Shown>);

    impl View for Record {
        fn show(&mut self, shown: Shown) -> io::Result<()> {
            self.0.push(shown);
            Ok(())
        }
    }

    fn lines(narrated: &mut Narrated<Record>) -> Vec<String> {
        let shown = std::mem::take(&mut narrated.view.0);
        shown
            .into_iter()
            .filter_map(|shown| match shown {
                Shown::Line(message) => Some(message.plain()),
                _ => None,
            })
            .collect()
    }

    fn status(trace: Option<u32>, integrity: u32, heat: u32) -> Status {
        Status {
            host: "gate".to_owned(),
            run: trace.map(|trace| RunStatus {
                network: "Test Lab".to_owned(),
                access: "guest".to_owned(),
                trace,
            }),
            integrity,
            heat,
            heat_max: 5,
            ..Status::default()
        }
    }

    #[test]
    fn verbose_tells_each_change_of_state_after_the_first_status() {
        let mut narrated = Narrated::new(Record::default());
        let mut told = |status: Status| {
            narrated.show(Shown::Status(status)).unwrap();
            lines(&mut narrated)
        };
        assert_eq!(told(status(None, 100, 0)), Vec::<String>::new());
        assert_eq!(told(status(Some(0), 100, 0)), ["You are now in Test Lab."]);
        assert_eq!(
            told(status(Some(14), 80, 1)),
            [
                "Trace is now 14 percent.",
                "Integrity is now 80.",
                "Heat is now 1 of 5."
            ]
        );
        let mut user = status(Some(14), 80, 1);
        if let Some(run) = user.run.as_mut() {
            run.access = "user".to_owned();
        }
        assert_eq!(told(user), ["Access is now user."]);
        assert_eq!(told(status(None, 80, 1)), ["You left Test Lab."]);
    }

    #[test]
    fn verbose_starts_each_telegraph_with_alert() {
        let mut narrated = Narrated::new(Record::default());
        let telegraph = Message::new(Kind::Telegraph)
            .span(Style::Ice, "hound")
            .text(" starts to compile TRACE BURST.");
        narrated.show(Shown::Line(telegraph)).unwrap();
        narrated
            .show(Shown::Line(Message::game("The Relay")))
            .unwrap();
        assert_eq!(
            lines(&mut narrated),
            ["Alert: hound starts to compile TRACE BURST.", "The Relay"]
        );
    }
}
