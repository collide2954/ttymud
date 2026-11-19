use crate::content::File;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Look,
    List,
    Go,
    Back,
    Read,
    Download,
    Inventory,
    Status,
    Jackout,
    Repair,
    Load,
    Unload,
    Stash,
    Unstash,
    Contract,
    Jobs,
    Accept,
    Shop,
    Buy,
    Sell,
    Scan,
    Probe,
    Crack,
    Scrub,
    Patch,
    Firewall,
    Mask,
    Proxy,
    Pretext,
    Exploit,
    Grep,
    Say,
    Channels,
    Join,
    Leave,
    Who,
    Tell,
    Reply,
    Ignore,
    Unignore,
    Block,
    Unblock,
    Report,
    Recovery,
    Link,
    Keys,
    Revoke,
    Mute,
    Unmute,
    Kick,
    Ban,
    Unban,
    Bans,
    Reports,
    Resolve,
    Help,
    Available,
}

const SETUP: u32 = 1;
const CONNECTION: u32 = 1;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Program {
    pub(crate) ram: u32,
    pub(crate) cycles: u32,
    pub(crate) noise: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cost {
    pub(crate) cycles: u32,
    pub(crate) noise: u32,
}

impl Verb {
    pub(crate) fn takes_args(self) -> bool {
        !matches!(
            self,
            Self::Look
                | Self::List
                | Self::Back
                | Self::Inventory
                | Self::Status
                | Self::Jackout
                | Self::Repair
                | Self::Jobs
                | Self::Shop
                | Self::Scan
                | Self::Scrub
                | Self::Patch
                | Self::Firewall
                | Self::Mask
                | Self::Proxy
                | Self::Exploit
                | Self::Channels
                | Self::Who
                | Self::Recovery
                | Self::Link
                | Self::Keys
                | Self::Bans
                | Self::Available
        )
    }

    pub(crate) fn moderation(self) -> bool {
        matches!(
            self,
            Self::Mute
                | Self::Unmute
                | Self::Kick
                | Self::Ban
                | Self::Unban
                | Self::Bans
                | Self::Reports
                | Self::Resolve
        )
    }

    pub(crate) fn program(self) -> Option<Program> {
        let (ram, cycles, noise) = match self {
            Self::Scan | Self::Probe => (1, 1, 1),
            Self::Crack | Self::Pretext => (2, 2, 2),
            Self::Scrub | Self::Patch | Self::Firewall | Self::Mask => (2, 2, 0),
            Self::Proxy => (2, 3, 1),
            Self::Exploit => (2, 3, 3),
            _ => return None,
        };
        Some(Program { ram, cycles, noise })
    }
}

pub(crate) fn cost(programs: impl IntoIterator<Item = Program>) -> Cost {
    let mut programs = programs.into_iter().peekable();
    if programs.peek().is_none() {
        return Cost::default();
    }
    let start = Cost {
        cycles: SETUP,
        noise: 0,
    };
    let mut cost = programs.fold(start, |cost, program| Cost {
        cycles: cost.cycles + program.cycles,
        noise: cost.noise + program.noise,
    });
    if cost.noise > 0 {
        cost.noise += CONNECTION;
    }
    cost
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    Route(String),
    File(File),
    Ice { name: String, weak: bool },
    Line(String),
}

impl Item {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Route(id) => id,
            Self::File(file) => &file.name,
            Self::Ice { name, .. } | Self::Line(name) => name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Kinds(u8);

impl Kinds {
    const NONE: Self = Self(0);
    const ROUTES: Self = Self(1);
    const FILES: Self = Self(2);
    const PLACES: Self = Self(3);
    const LINES: Self = Self(4);
    const ICE: Self = Self(8);
    const ANY: Self = Self(15);

    fn overlaps(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    fn describe(self) -> String {
        let words: Vec<&str> = [
            (Self::ROUTES, "routes"),
            (Self::FILES, "files"),
            (Self::LINES, "lines"),
            (Self::ICE, "ICE"),
        ]
        .into_iter()
        .filter(|&(kind, _)| self.overlaps(kind))
        .map(|(_, word)| word)
        .collect();
        match words.as_slice() {
            [] => "nothing".to_owned(),
            [rest @ .., last] if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            _ => words.join(""),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Takes {
    Nothing,
    Optional(Kinds),
    Required(Kinds),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Gives {
    Nothing,
    Items(Kinds),
    Same,
}

#[derive(Debug)]
pub(crate) struct Command {
    pub(crate) verb: Verb,
    pub(crate) names: &'static [&'static str],
    pub(crate) usage: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) example: &'static str,
    pub(crate) takes: Takes,
    pub(crate) gives: Gives,
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        self.names[0]
    }
}

pub(crate) const COMMANDS: &[Command] = &[
    Command {
        verb: Verb::Look,
        names: &["look", "l"],
        usage: "look",
        summary: "Show the host where you are.",
        example: "look",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::PLACES),
    },
    Command {
        verb: Verb::List,
        names: &["ls"],
        usage: "ls",
        summary: "List the routes and files here.",
        example: "ls",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::PLACES),
    },
    Command {
        verb: Verb::Go,
        names: &["go", "cd", "connect", "ssh"],
        usage: "go <place>",
        summary: "Move to a host through a route.",
        example: "go market",
        takes: Takes::Optional(Kinds::ROUTES),
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Back,
        names: &["back", "exit"],
        usage: "back",
        summary: "Move back one hop. cd .. does the same.",
        example: "back",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Read,
        names: &["read", "cat"],
        usage: "read <file>",
        summary: "Show the text of a file here or on your rig.",
        example: "read motd.txt",
        takes: Takes::Optional(Kinds::FILES),
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Download,
        names: &["download", "get"],
        usage: "download <file>",
        summary: "Copy a file from this host to your rig.",
        example: "download motd.txt",
        takes: Takes::Optional(Kinds::FILES),
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Inventory,
        names: &["inventory", "inv", "i"],
        usage: "inventory",
        summary: "Show the files on your rig.",
        example: "inventory",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::FILES),
    },
    Command {
        verb: Verb::Status,
        names: &["status", "st"],
        usage: "status",
        summary: "Show your heat, trace, integrity, cycles, and RAM.",
        example: "status",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Jackout,
        names: &["jackout"],
        usage: "jackout",
        summary: "Drop the link to the target at once. It costs integrity.",
        example: "jackout",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Repair,
        names: &["repair"],
        usage: "repair",
        summary: "Restore your integrity at home. Up to 50 is free. Above 50, each point costs 2 cr.",
        example: "repair",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Load,
        names: &["load"],
        usage: "load <program>",
        summary: "Put a program that you know into RAM. This works only at home.",
        example: "load patch",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Unload,
        names: &["unload"],
        usage: "unload <program>",
        summary: "Remove a program from RAM. This works only at home.",
        example: "unload patch",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Stash,
        names: &["stash"],
        usage: "stash [file]",
        summary: "Show your stash, or put a file into it at home. A burn does not take it.",
        example: "stash patients.db",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Unstash,
        names: &["unstash"],
        usage: "unstash <file>",
        summary: "Take a file from your stash. This works only at home.",
        example: "unstash patients.db",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Contract,
        names: &["contract", "job"],
        usage: "contract [drop]",
        summary: "Show your job, or drop it.",
        example: "contract",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Jobs,
        names: &["jobs"],
        usage: "jobs",
        summary: "Show the jobs of the fixers here.",
        example: "jobs",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Accept,
        names: &["accept"],
        usage: "accept <number>",
        summary: "Take a job from a fixer here.",
        example: "accept 1",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Shop,
        names: &["shop"],
        usage: "shop",
        summary: "Show what the shop here sells.",
        example: "shop",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Buy,
        names: &["buy"],
        usage: "buy <item>",
        summary: "Buy a program or a rig upgrade at a shop.",
        example: "buy ram",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Sell,
        names: &["sell"],
        usage: "sell <file>",
        summary: "Sell stolen data to the fence at a shop.",
        example: "inventory | grep .db | sell",
        takes: Takes::Optional(Kinds::FILES),
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Scan,
        names: &["scan"],
        usage: "scan",
        summary: "Find the ICE on this host.",
        example: "scan",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::ICE),
    },
    Command {
        verb: Verb::Probe,
        names: &["probe"],
        usage: "probe <ice>",
        summary: "Find the weak point of an ICE.",
        example: "probe sentry",
        takes: Takes::Optional(Kinds::ICE),
        gives: Gives::Items(Kinds::ICE),
    },
    Command {
        verb: Verb::Crack,
        names: &["crack"],
        usage: "crack <ice>",
        summary: "Attack an ICE. At zero integrity, the ICE breaks.",
        example: "crack sentry",
        takes: Takes::Optional(Kinds::ICE),
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Scrub,
        names: &["scrub"],
        usage: "scrub",
        summary: "Delete your log lines to lower the trace.",
        example: "scrub",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Patch,
        names: &["patch"],
        usage: "patch",
        summary: "Repair some integrity during a run.",
        example: "patch",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Firewall,
        names: &["firewall"],
        usage: "firewall",
        summary: "Stop the next attack on your integrity.",
        example: "firewall",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Mask,
        names: &["mask"],
        usage: "mask",
        summary: "Hide your noise from sentries for 5 ticks.",
        example: "mask",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Proxy,
        names: &["proxy"],
        usage: "proxy",
        summary: "Halve the ICE attacks on your trace for this run.",
        example: "proxy",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Pretext,
        names: &["pretext"],
        usage: "pretext <warden>",
        summary: "Talk your way past a warden.",
        example: "pretext warden",
        takes: Takes::Optional(Kinds::ICE),
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Exploit,
        names: &["exploit"],
        usage: "exploit",
        summary: "Raise your access on this host by one level. It makes much noise.",
        example: "exploit",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Grep,
        names: &["grep"],
        usage: "grep <pattern>",
        summary: "Keep only the items that contain the pattern.",
        example: "ls | grep txt",
        takes: Takes::Required(Kinds::ANY),
        gives: Gives::Same,
    },
    Command {
        verb: Verb::Say,
        names: &["say", "'"],
        usage: "say <text>",
        summary: "Talk to everyone on this host.",
        example: "say hello",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Channels,
        names: &["channels"],
        usage: "channels",
        summary: "Show the chat channels. To talk in one, type #<channel> <text>.",
        example: "#ostra hello",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Join,
        names: &["join"],
        usage: "join <channel>",
        summary: "Join a chat channel.",
        example: "join market",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Leave,
        names: &["leave"],
        usage: "leave <channel>",
        summary: "Leave a chat channel.",
        example: "leave market",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Who,
        names: &["who"],
        usage: "who",
        summary: "Show the handles that are online.",
        example: "who",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Tell,
        names: &["tell"],
        usage: "tell <handle> <text>",
        summary: "Send a private message to a handle that is online.",
        example: "tell nyx meet me at the relay",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Reply,
        names: &["reply"],
        usage: "reply <text>",
        summary: "Answer the last handle that sent you a private message.",
        example: "reply on my way",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Ignore,
        names: &["ignore"],
        usage: "ignore [handle]",
        summary: "Hide the messages of a handle, or show whom you ignore and block.",
        example: "ignore zer0",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Unignore,
        names: &["unignore"],
        usage: "unignore <handle>",
        summary: "See the messages of a handle again.",
        example: "unignore zer0",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Block,
        names: &["block"],
        usage: "block <handle>",
        summary: "Hide the messages of a handle and refuse its private messages.",
        example: "block zer0",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Unblock,
        names: &["unblock"],
        usage: "unblock <handle>",
        summary: "Take private messages from a handle again.",
        example: "unblock zer0",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Report,
        names: &["report"],
        usage: "report <handle> [reason]",
        summary: "Send a handle and its chat from the last 15 minutes to the moderators.",
        example: "report zer0 spam",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Recovery,
        names: &["recovery"],
        usage: "recovery",
        summary: "Make a new recovery code. On a new device, type recover <code> at the handle prompt.",
        example: "recovery",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Link,
        names: &["link"],
        usage: "link",
        summary: "Make a code that adds a new device to your account. On the new device, type link <code> at the handle prompt.",
        example: "link",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Keys,
        names: &["keys"],
        usage: "keys",
        summary: "Show the keys of the devices of your account.",
        example: "keys",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Revoke,
        names: &["revoke"],
        usage: "revoke <number>",
        summary: "Remove a key from your account. Its device cannot connect again.",
        example: "revoke 2",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Mute,
        names: &["mute"],
        usage: "mute <handle> <minutes>",
        summary: "Moderators: stop the chat messages of a handle for some minutes.",
        example: "mute zer0 30",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Unmute,
        names: &["unmute"],
        usage: "unmute <handle>",
        summary: "Moderators: let a muted handle send chat messages again.",
        example: "unmute zer0",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Kick,
        names: &["kick"],
        usage: "kick <handle>",
        summary: "Moderators: disconnect a handle from the server.",
        example: "kick zer0",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Ban,
        names: &["ban"],
        usage: "ban <handle> [ip]",
        summary: "Moderators: disconnect a handle and refuse its account. With ip, also refuse its IP address.",
        example: "ban zer0",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Unban,
        names: &["unban"],
        usage: "unban <handle>",
        summary: "Moderators: accept a banned account again.",
        example: "unban zer0",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Bans,
        names: &["bans"],
        usage: "bans",
        summary: "Moderators: show the banned handles.",
        example: "bans",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Reports,
        names: &["reports"],
        usage: "reports [number]",
        summary: "Moderators: show the open reports, or one report with its chat.",
        example: "reports 3",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Resolve,
        names: &["resolve"],
        usage: "resolve <number>",
        summary: "Moderators: close a report.",
        example: "resolve 3",
        takes: Takes::Nothing,
        gives: Gives::Nothing,
    },
    Command {
        verb: Verb::Help,
        names: &["help", "man"],
        usage: "help [command]",
        summary: "Show help for all commands or for one command.",
        example: "help go",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
    Command {
        verb: Verb::Available,
        names: &["?"],
        usage: "?",
        summary: "Show the commands that you can use here.",
        example: "?",
        takes: Takes::Nothing,
        gives: Gives::Items(Kinds::LINES),
    },
];

pub(crate) fn names() -> impl Iterator<Item = &'static str> {
    COMMANDS
        .iter()
        .flat_map(|command| command.names.iter().copied())
}

pub(crate) fn programs() -> impl Iterator<Item = (&'static Command, Program)> {
    COMMANDS
        .iter()
        .filter_map(|command| command.verb.program().map(|program| (command, program)))
}
pub(crate) fn name_of(verb: Verb) -> &'static str {
    COMMANDS
        .iter()
        .find(|command| command.verb == verb)
        .map_or("", Command::name)
}

pub(crate) fn summary_of(verb: Verb) -> &'static str {
    COMMANDS
        .iter()
        .find(|command| command.verb == verb)
        .map_or("", |command| command.summary)
}

pub(crate) fn find(name: &str) -> Option<&'static Command> {
    let name = name.to_lowercase();
    COMMANDS
        .iter()
        .find(|command| command.names.contains(&name.as_str()))
}

pub(crate) fn check(commands: &[&Command]) -> Result<(), String> {
    let mut flow: Option<(Kinds, &str)> = None;
    for (index, command) in commands.iter().enumerate() {
        let name = command.name();
        match (command.takes, flow) {
            (Takes::Nothing, Some(_)) => return Err(format!("{name} takes no input from a pipe.")),
            (Takes::Required(_), None) => {
                let example = command.example;
                return Err(format!(
                    "{name} needs input from a pipe. Example: {example}"
                ));
            }
            (Takes::Optional(wanted) | Takes::Required(wanted), Some((kinds, from)))
                if !wanted.overlaps(kinds) =>
            {
                let (wanted, kinds) = (wanted.describe(), kinds.describe());
                return Err(format!("{name} needs {wanted}, but {from} gives {kinds}."));
            }
            _ => {}
        }
        flow = match command.gives {
            Gives::Nothing if index + 1 < commands.len() => {
                return Err(format!("{name} gives no output to a pipe."));
            }
            Gives::Nothing => Some((Kinds::NONE, name)),
            Gives::Items(kinds) => Some((kinds, name)),
            Gives::Same => flow,
        };
    }
    Ok(())
}
