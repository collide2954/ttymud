use ttymud_text::{Kind, Message, Style};

pub(super) const TOPICS: &[&str] = &[
    "alert",
    "burn",
    "heat",
    "ice",
    "patches",
    "pipes",
    "pipelines",
    "runs",
    "soc",
    "trace",
];

pub(super) fn topic(name: &str) -> Option<Vec<Message>> {
    match name {
        "heat" | "burn" => Some(heat()),
        "ice" => Some(ice()),
        "pipes" | "pipelines" => Some(pipes()),
        "runs" | "trace" => Some(runs()),
        "soc" | "alert" | "patches" => Some(soc()),
        _ => None,
    }
}

pub(super) fn index() -> Message {
    Message::game("Topics: heat, ice, pipes, runs, soc. Example: ").span(Style::Command, "help ice")
}

fn soc() -> Vec<Message> {
    let command = |text: &str| Message::new(Kind::Game).span(Style::Command, text);
    vec![
        title("The SOC"),
        Message::game("Each corp has an alert level: green, yellow, orange, or red."),
        Message::game(
            "The noise, traces, and breaches of all players raise it. It cools by 1 point every 5 minutes.",
        ),
        Message::game(
            "A higher alert adds trace from noise, analysts on shift, and stronger ICE in new runs.",
        ),
        Message::game(
            "An analyst reads the logs of one host. If your noise is there, your trace goes up.",
        ),
        Message::game(
            "A careful or strict analyst also locks your stolen account. Then your access is guest again.",
        ),
        Message::game("Type ")
            .span(Style::Command, "scrub")
            .text(" to delete your logs on this host. While ")
            .span(Style::Command, "mask")
            .text(" is up, your noise stays out of the logs."),
        command("pretext <analyst>").text(
            " makes an analyst stop watching your run. A strict analyst does not believe you.",
        ),
        Message::game(
            "A corp patches software after many exploits. Later, somebody finds a new hole.",
        ),
        Message::game("Type ")
            .span(Style::Command, "news")
            .text(" to read the news. At a fixer, type ")
            .span(Style::Command, "rumors")
            .text(" to hear rumors."),
    ]
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

fn heat() -> Vec<Message> {
    vec![
        title("Heat"),
        Message::game("Heat shows how much the police know about your handle, from 0 to 5."),
        Message::game("A trace adds 1 to 5 heat. A target with more security gives more heat."),
        Message::game("A jack out with a trace of 75% or more adds 1 heat."),
        Message::game("At heat 3 or more, a sentry sends its ALERT faster."),
        Message::game("Heat goes down by 1 for each day without new heat."),
        Message::game(
            "At heat 5, your handle burns. Your account keeps its know-how and its stash.",
        ),
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
        Message::game("Some files need user or admin access. Read stolen credentials, or run ")
            .span(Style::Command, "exploit")
            .text("."),
        Message::game("A run ends in one of four ways:"),
        Message::game("Clean exit: leave the way that you came in. You keep the loot."),
        Message::game("Jack out: type ")
            .span(Style::Command, "jackout")
            .text(". You keep the loot, but you lose 20 integrity."),
        Message::game("Traced: the trace gets to 100%. You lose the loot of the run and get heat."),
        Message::game("Crash: your integrity gets to 0. You lose the loot. Type ")
            .span(Style::Command, "repair")
            .text(" at home."),
    ]
}
