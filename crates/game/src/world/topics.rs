use ttymud_text::{Kind, Message, Style};

pub(super) const TOPICS: &[&str] = &[
    "alert",
    "arena",
    "auction",
    "blueprints",
    "burn",
    "crews",
    "darknet",
    "economy",
    "heat",
    "heist",
    "ice",
    "patches",
    "pipes",
    "pipelines",
    "pvp",
    "runs",
    "script",
    "scripts",
    "soc",
    "trace",
];

pub(super) fn topic(name: &str) -> Option<Vec<Message>> {
    match name {
        "crews" | "heist" => Some(crews()),
        "economy" | "auction" | "blueprints" => Some(economy()),
        "heat" | "burn" => Some(heat()),
        "ice" => Some(ice()),
        "pipes" | "pipelines" => Some(pipes()),
        "pvp" | "darknet" | "arena" => Some(pvp()),
        "runs" | "trace" => Some(runs()),
        "scripts" | "script" => Some(scripts()),
        "soc" | "alert" | "patches" => Some(soc()),
        _ => None,
    }
}

pub(super) fn index() -> Message {
    Message::game("Topics: crews, economy, heat, ice, pipes, pvp, runs, scripts, soc. Example: ")
        .span(Style::Command, "help ice")
}

fn scripts() -> Vec<Message> {
    let command = |text: &str| Message::new(Kind::Game).span(Style::Command, text);
    vec![
        title("Scripts"),
        Message::game(
            "A script is a file on your rig with one command on each line. Its name ends in ",
        )
        .span(Style::File, ".sh")
        .text("."),
        command("echo 'scan | probe' > sweep.sh").text(" writes a script, and >> adds a line."),
        command("sh sweep.sh sentry").text(" runs it. The words after the name go into $1 to $9."),
        Message::game(
            "A script runs one command on each tick. It waits until your rig has the cycles for the command.",
        ),
        Message::game("A running script uses 1 GB of RAM. Type ")
            .span(Style::Command, "ps")
            .text(" to see it, and ")
            .span(Style::Command, "kill")
            .text(" to stop it."),
        Message::game(
            "&& runs the next command only if the last command worked. || runs it only if the last command failed.",
        ),
        Message::game(
            "If the last command of a line fails, the script stops. A crash, a trace, or a burn also stops it.",
        ),
        Message::game("grep fails when nothing matches. ")
            .span(Style::Command, "scan | grep sentry && crack sentry")
            .text(" cracks the sentry only if it is there."),
        command("wait 3").text(" lets 3 ticks go by. A line that starts with # is a note."),
        Message::game("A script cannot chat, send a challenge, or start a script."),
        Message::game("Example: ").span(Style::Command, "echo 'crack $1 || jackout' >> sweep.sh"),
    ]
}

fn pvp() -> Vec<Message> {
    let command = |text: &str| Message::new(Kind::Game).span(Style::Command, text);
    vec![
        title("Fights between handles"),
        Message::game(
            "Handles can attack each other only in a darknet, in a duel, or in a crew match.",
        ),
        Message::game("In a darknet, noise gives no trace. ")
            .span(Style::Command, "probe <handle> | crack")
            .text(" attacks a rig on your host."),
        Message::game(
            "A crash in a darknet leaves your data and blueprints on the host, and you wake up at home.",
        ),
        command("tip <handle>")
            .text(" gives 1 heat to a handle in your darknet. A handle gets one tip in 24 hours."),
        command("duel <handle> [credits]").text(" challenges a handle to a fight in the arena."),
        Message::game("A crew leader types ")
            .span(Style::Command, "match <crew> [credits]")
            .text(" for capture the flag. 3 captures win."),
        Message::game("A fight gives no heat. When it ends, your rig is as it was before."),
    ]
}

fn economy() -> Vec<Message> {
    vec![
        title("The economy"),
        Message::game(
            "Six programs have versions 1 to 3. A higher version makes the main effect stronger.",
        ),
        Message::game("Read a blueprint file, for example ")
            .span(Style::File, "crack-v2.blueprint")
            .text(", to learn it. Targets, vaults, and shops have blueprints."),
        Message::game("At home, type ")
            .span(Style::Command, "compile <program>")
            .text(" to make the next version: 1000 cr for v2, and 3000 cr for v3."),
        Message::game("Type ")
            .span(Style::Command, "compile <program> quiet")
            .text(" to add the quiet mod for 400 cr: 1 less noise and 1 more cycle."),
        Message::game(
            "Each sale of a file name lowers its price by 10%, to 30%. The price goes up 10% every 2 hours.",
        ),
        Message::game("The police auction the rigs of burned handles. At the auction host, type ")
            .span(Style::Command, "lots")
            .text(" and ")
            .span(Style::Command, "bid <lot> <credits>")
            .text("."),
        Message::game(
            "After 3 days, the top bid wins the lot. A bidder who loses the top bid gets the credits back.",
        ),
    ]
}

fn crews() -> Vec<Message> {
    let command = |text: &str| Message::new(Kind::Game).span(Style::Command, text);
    vec![
        title("Crews and heists"),
        Message::game("A crew has up to 4 handles. Type ")
            .span(Style::Command, "crew create <name>")
            .text(", then ")
            .span(Style::Command, "crew invite <handle>")
            .text("."),
        command("#crew <text>").text(" reaches your crewmates on all hosts."),
        Message::game(
            "Crewmates on the same job share one copy of the network, with its ICE, analysts, and logs.",
        ),
        Message::game("Each crewmate keeps a separate trace. ")
            .span(Style::Command, "scrub <handle>")
            .text(" helps a crewmate on your host. ")
            .span(Style::Command, "decoy")
            .text(" draws the ICE on your host to you."),
        Message::game(
            "Each fixer posts a heist each day for a crew of 2 to 4. Only the leader can accept it.",
        ),
        Message::game(
            "The goal is in the vault. Wait on the maintenance laptop for its sync, or let a Face or a Phreak open a bridge.",
        ),
        Message::game("A clean exit with the goal pays each crewmate who entered the target."),
    ]
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
        Message::game(
            "The analyst also resets the stolen password. Those credentials do not work until the next day.",
        ),
        Message::game(
            "A strict analyst can also cut one route that leads deeper. The cut lasts until the run ends.",
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
        kind(
            "tarpit",
            "Noise wakes it. Its TAR takes all your cycles. Leave its host to stop it.",
        ),
        kind(
            "lockjaw",
            "Noise wakes it. Its BITE freezes the program that you used last, for 6 ticks.",
        ),
        kind(
            "honeypot",
            "A bait file. If you read or download it, your trace goes up by 30%. Type probe <file> to find it.",
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
