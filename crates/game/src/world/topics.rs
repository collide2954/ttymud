use ttymud_text::{Kind, Message, Style};

pub(super) const TOPICS: &[&str] = &["ice", "pipes", "pipelines", "runs", "trace"];

pub(super) fn topic(name: &str) -> Option<Vec<Message>> {
    match name {
        "ice" => Some(ice()),
        "pipes" | "pipelines" => Some(pipes()),
        "runs" | "trace" => Some(runs()),
        _ => None,
    }
}

pub(super) fn index() -> Message {
    Message::game("Topics: ice, pipes, runs. Example: ").span(Style::Command, "help ice")
}

fn title(text: &str) -> Message {
    Message::new(Kind::Game).span(Style::Title, text)
}

fn kind(name: &str, text: &str) -> Message {
    Message::new(Kind::Game)
        .span(Style::Ice, name)
        .text(format!(": {text}"))
}

fn pipes() -> Vec<Message> {
    vec![
        title("Pipelines"),
        Message::game("Join commands with ")
            .span(Style::Command, "|")
            .text(" to send the items of one command to the next."),
        Message::game("Only the last command shows its output."),
        Message::game("Programs in one pipeline share one setup cycle and one connection."),
        Message::game("Example: ").span(Style::Command, "ls | grep txt | read"),
    ]
}

fn ice() -> Vec<Message> {
    vec![
        title("ICE"),
        Message::game("ICE is security software in a target network. Type ")
            .span(Style::Command, "scan")
            .text(" to find it."),
        Message::game("Before an ICE acts, it shows its next action and the ticks that are left."),
        kind(
            "sentry",
            "It watches for noise. It doubles your trace and sends an ALERT.",
        ),
        kind("warden", "It blocks one route until you break it."),
        kind(
            "hound",
            "An ALERT wakes it. It follows you and attacks your trace.",
        ),
        kind(
            "scorch",
            "Noise wakes it. It attacks your integrity. Leave its host to stop the attack.",
        ),
        Message::game("A crack after a probe in the same pipeline hits the weak point: ")
            .span(Style::Command, "probe sentry | crack"),
    ]
}

fn runs() -> Vec<Message> {
    vec![
        title("Runs"),
        Message::game("A run starts when you connect to a target network. The trace starts at 0%."),
        Message::game("Noise from your programs raises the trace. ")
            .span(Style::Command, "scrub")
            .text(" lowers it."),
        Message::game("A run ends in one of four ways:"),
        Message::game("Clean exit: leave the way that you came in. You keep the loot."),
        Message::game("Jack out: type ")
            .span(Style::Command, "jackout")
            .text(". You keep the loot, but you lose 20 integrity."),
        Message::game("Traced: the trace gets to 100%. You lose the loot of the run."),
        Message::game("Crash: your integrity gets to 0. You lose the loot. Type ")
            .span(Style::Command, "repair")
            .text(" at home."),
    ]
}
