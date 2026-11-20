use super::{
    ChannelDef, Content, ContractDef, CorpDef, File, FixerDef, IceDef, NetworkDef, ShopDef,
    TemplateDef,
};
use crate::commands::Verb;
use crate::contract::Event;
use crate::heat::Security;
use crate::ice::IceKind;
use crate::target::Layer;

const NET: &str = "\
[city t]
name = T
start = a
[host a]
name = A
desc = A.
[network n]
name = N
[host b]
name = B
desc = B.
network = n
route = a
route = c
[host c]
name = C
desc = C.
network = n
";

#[test]
fn the_example_in_the_content_document_is_valid() {
    let document = include_str!("../../../../docs/content.md");
    let section = document.split("## Example").nth(1).unwrap_or_default();
    let lines: Vec<&str> = section
        .split("\n## ")
        .next()
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.strip_prefix("    "))
        .collect();
    let example = lines.join("\n");
    assert!(example.contains("[contract first-job]"), "{example}");
    assert_eq!(Content::parse(&example).err(), None);
}

#[test]
fn the_city_can_name_a_host_for_the_burn_list() {
    let text = NET.replace("start = a", "start = a\nburns = a");
    assert_eq!(Content::parse(&text).unwrap().burns.as_deref(), Some("a"));
    assert_eq!(
        errors(&NET.replace("start = a", "start = a\nburns = x")),
        ["line 4: burns is an unknown host: x"]
    );
    assert_eq!(
        errors(&NET.replace("start = a", "start = a\nburns = b")),
        ["line 4: burns is in a network: b"]
    );
}

#[test]
fn the_city_can_name_a_host_for_the_news() {
    let text = NET.replace("start = a", "start = a\nnews = a");
    assert_eq!(Content::parse(&text).unwrap().news.as_deref(), Some("a"));
    assert_eq!(
        errors(&NET.replace("start = a", "start = a\nnews = x")),
        ["line 4: news is an unknown host: x"]
    );
}

#[test]
fn a_network_has_a_security_level() {
    let text = NET.replace("name = N\n", "name = N\nsecurity = high\n");
    let content = Content::parse(&text).unwrap();
    assert_eq!(content.networks[0].security, Security::High);
    assert_eq!(
        Content::parse(NET).unwrap().networks[0].security,
        Security::Low
    );
    let text = NET.replace("name = N\n", "name = N\nsecurity = extreme\n");
    assert_eq!(
        errors(&text),
        ["line 9: unknown security level: extreme (use low, medium, high, or top)"]
    );
}

#[test]
fn a_channel_has_a_description_and_can_be_automatic() {
    let text = format!(
        "{NET}[channel city]\ndesc = All of the city.\nauto = yes\n[channel trade]\ndesc = Buy and sell.\n"
    );
    let channel = |id: &str, desc: &str, auto| ChannelDef {
        id: id.to_owned(),
        desc: desc.to_owned(),
        auto,
    };
    assert_eq!(
        Content::parse(&text).unwrap().channels,
        [
            channel("city", "All of the city.", true),
            channel("trade", "Buy and sell.", false)
        ]
    );
    assert_eq!(
        errors(&text.replace("auto = yes", "auto = maybe")),
        ["line 21: bad auto: maybe (use yes or no)"]
    );
    assert_eq!(
        errors(&format!("{text}[channel city]\ndesc = Again.\n")),
        ["line 24: channel city is defined more than once"]
    );
    assert_eq!(
        errors(&format!("{NET}[channel crew]\ndesc = Mine.\n")),
        ["line 19: the channel name crew is for crews"]
    );
}

#[test]
fn a_shop_sells_programs_and_hardware() {
    let text =
        format!("{NET}[shop a]\nprogram = mask 400\nram = 12 900\nram = 16 1800\ncpu = 5 1200\n");
    let shop = ShopDef {
        host: "a".to_owned(),
        programs: vec![(Verb::Mask, 400)],
        ram: vec![(12, 900), (16, 1800)],
        cpu: vec![(5, 1200)],
    };
    assert_eq!(Content::parse(&text).unwrap().shops, [shop]);
}

#[test]
fn bad_shops_fail() {
    let text = format!(
        "{NET}[shop x]\n[shop b]\n[shop a]\nprogram = nuke 5\nprogram = mask\n\
         ram = 6 100\ncpu = 5 lots\n[shop a]\n"
    );
    assert_eq!(
        errors(&text),
        [
            "line 19: shop is an unknown host: x",
            "line 20: shop is in a network: b",
            "line 22: unknown program: nuke",
            "line 23: program needs a name and a price, for example program = mask 400",
            "line 24: each ram step must be more than 8 GB: 6",
            "line 25: bad price: lots (use a number of credits)",
            "line 26: shop a is defined more than once",
        ]
    );
}

const TEMPLATES: &str = "\
[template vpn]\nlayer = edge\nname = V\ndesc = V.\n\
[template ws]\nlayer = lan\nname = W\ndesc = W.\n\
[template db]\nlayer = core\nname = D\ndesc = D.\n";

#[test]
fn a_corp_has_a_budget_and_data() {
    let text = format!(
        "{NET}{TEMPLATES}[corp kh]\nname = Kestrel Health\nbudget = medium\n\
         data = claims.db: Ten years of claims.\ndata = patients.db: Patient files.\n"
    );
    let corp = CorpDef {
        id: "kh".to_owned(),
        name: "Kestrel Health".to_owned(),
        budget: Security::Medium,
        data: vec![
            File::new("claims.db", vec!["Ten years of claims.".to_owned()]),
            File::new("patients.db", vec!["Patient files.".to_owned()]),
        ],
    };
    assert_eq!(Content::parse(&text).unwrap().corps, [corp]);
}

#[test]
fn a_template_makes_hosts_for_one_layer() {
    let text = format!(
        "{NET}[template vpn]\nlayer = edge\nname = VPN Gateway\ndesc = An old box.\n\
         software = Gatekeep VPN\n[file vpn/motd.txt]\nline = Hi.\n"
    );
    let template = TemplateDef {
        id: "vpn".to_owned(),
        layer: Layer::Edge,
        name: "VPN Gateway".to_owned(),
        description: "An old box.".to_owned(),
        software: Some("Gatekeep VPN".to_owned()),
        laptop: false,
        files: vec![File::new("motd.txt", vec!["Hi.".to_owned()])],
    };
    assert_eq!(Content::parse(&text).unwrap().templates, [template]);
    let plain = Content::parse(&format!("{NET}{TEMPLATES}")).unwrap();
    assert!(plain.templates.iter().all(|def| def.software.is_none()));
}

#[test]
fn a_vault_template_needs_a_laptop_template_on_the_lan() {
    let laptop = "[template maint]\nlayer = lan\nname = M\ndesc = M.\nlaptop = yes\n";
    let vault = "[template safe]\nlayer = vault\nname = S\ndesc = S.\n";
    let content = Content::parse(&format!("{NET}{TEMPLATES}{laptop}{vault}")).unwrap();
    let kinds: Vec<(Layer, bool)> = content
        .templates
        .iter()
        .map(|def| (def.layer, def.laptop))
        .collect();
    assert_eq!(kinds[3..], [(Layer::Lan, true), (Layer::Vault, false)]);
    assert_eq!(
        errors(&format!("{NET}{TEMPLATES}{vault}")),
        ["line 31: a vault template needs a laptop template"]
    );
    let bad = "[template x]\nlayer = core\nname = X\ndesc = X.\nlaptop = yes\n\
               [template y]\nlayer = lan\nname = Y\ndesc = Y.\nlaptop = maybe\n";
    assert_eq!(
        errors(&format!("{NET}{TEMPLATES}{bad}")),
        [
            "line 35: only a lan template can be a laptop",
            "line 40: bad laptop: maybe (use yes or no)"
        ]
    );
}

const CORPS: &str = "\
[corp lo]\nname = Lo\nbudget = low\ndata = a.db: A.\n\
[corp hi]\nname = Hi\nbudget = high\ndata = b.db: B.\n";

#[test]
fn a_fixer_works_against_corps_from_a_host() {
    let text = format!(
        "{NET}{TEMPLATES}{CORPS}[fixer pell]\nname = Pell\nhost = a\ncorp = lo\ncorp = hi\nmax-heat = 3\n"
    );
    let fixer = FixerDef {
        id: "pell".to_owned(),
        name: "Pell".to_owned(),
        host: "a".to_owned(),
        corps: vec!["lo".to_owned(), "hi".to_owned()],
        max_heat: Some(3),
    };
    assert_eq!(Content::parse(&text).unwrap().fixers, [fixer]);
}

#[test]
fn bad_fixers_fail() {
    let text = format!(
        "{NET}{TEMPLATES}{CORPS}[fixer x]\nname = X\nhost = b\ncorp = hi\ncorp = nope\nmax-heat = hot\n\
         [fixer y]\nname = Y\nhost = z\n"
    );
    assert_eq!(
        errors(&text),
        [
            "line 39: fixer x needs a corp with a low budget for new handles",
            "line 41: fixer x is on a host in a network: b",
            "line 43: fixer x works against an unknown corp: nope",
            "line 44: bad max-heat: hot (use a number from 0 to 4)",
            "line 47: fixer y is on an unknown host: z",
        ]
    );
}

#[test]
fn bad_corps_and_templates_fail() {
    let text = format!(
        "{NET}[corp kh]\nbudget = huge\ndata = claims.db\n[corp empty]\nname = E\nbudget = low\n\
         [template a]\nlayer = edge\nname = A\ndesc = A.\n\
         [template ws]\nlayer = attic\nname = WS\ndesc = WS.\n"
    );
    assert_eq!(
        errors(&text),
        [
            "the corps need a template for the lan layer",
            "the corps need a template for the core layer",
            "line 19: corp kh has no name",
            "line 20: unknown budget: huge (use low, medium, high, or top)",
            "line 21: data needs a file name and a line, for example data = claims.db: Ten years of claims.",
            "line 22: corp empty has no data",
            "line 25: template a has the same name as a host",
            "line 30: unknown layer: attic (use edge, lan, core, or vault)",
        ]
    );
}

fn with_contract(contract: &str) -> String {
    let city = NET.replace("start = a", "start = a\ncontract = job");
    format!("{city}[file c/x.db]\nline = X.\n{contract}[fixer pell]\nname = Pell\nhost = a\n")
}

#[test]
fn the_city_offers_a_contract() {
    let text = with_contract(
        "[contract job]\nfixer = pell\nnetwork = n\ngoal = x.db\noffer = Get x.db.\noffer = Go.\n",
    );
    let content = Content::parse(&text).unwrap();
    let job = ContractDef {
        id: "job".to_owned(),
        fixer: "pell".to_owned(),
        network: "n".to_owned(),
        goal: "x.db".to_owned(),
        offer: vec!["Get x.db.".to_owned(), "Go.".to_owned()],
        steps: Vec::new(),
        pay: 0,
    };
    assert_eq!(content.contracts, [job]);
    assert_eq!(content.contract.as_deref(), Some("job"));
}

#[test]
fn a_contract_names_a_known_fixer() {
    let text =
        with_contract("[contract job]\nfixer = nobody\nnetwork = n\ngoal = x.db\noffer = Go.\n");
    assert_eq!(
        errors(&text),
        ["line 23: contract job has an unknown fixer: nobody"]
    );
}

#[test]
fn a_contract_can_pay_credits() {
    let text = with_contract(
        "[contract job]\nfixer = pell\nnetwork = n\ngoal = x.db\noffer = Go.\npay = 250\n",
    );
    assert_eq!(Content::parse(&text).unwrap().contracts[0].pay, 250);
    assert_eq!(
        errors(&text.replace("pay = 250", "pay = lots")),
        ["line 27: bad pay: lots (use a number of credits)"]
    );
}

#[test]
fn contract_steps_have_an_event_and_a_hint() {
    let text = with_contract(
        "[contract job]\nfixer = pell\nnetwork = n\ngoal = x.db\noffer = Go.\n\
         step = enter: Connect to b.\nstep = exit: Leave.\n",
    );
    let content = Content::parse(&text).unwrap();
    assert_eq!(
        content.contracts[0].steps,
        [
            (Event::Enter, "Connect to b.".to_owned()),
            (Event::Exit, "Leave.".to_owned())
        ]
    );
}

#[test]
fn bad_steps_fail() {
    let text = with_contract(
        "[contract job]\nfixer = pell\nnetwork = n\ngoal = x.db\noffer = Go.\n\
         step = dance: No.\nstep = scan\n",
    );
    assert_eq!(
        errors(&text),
        [
            "line 27: unknown step: dance (use home, enter, scan, probe, crack, download, scrub, patch, or exit)",
            "line 28: a step needs an event and a hint, for example step = scan: Type scan.",
        ]
    );
}

#[test]
fn bad_contracts_fail() {
    let text = with_contract(
        "[contract job]\nfixer = pell\nnetwork = n\ngoal = y.db\noffer = Get y.db.\n\
         [contract far]\nfixer = pell\nnetwork = z\ngoal = x.db\noffer = Go.\n[contract bare]\n",
    );
    assert_eq!(
        errors(&text),
        [
            "line 25: goal y.db is not a file in network n",
            "line 29: contract far is for an unknown network: z",
            "line 32: contract bare has no fixer",
            "line 32: contract bare has no network",
            "line 32: contract bare has no goal",
            "line 32: contract bare has no offer",
        ]
    );
    let text = with_contract("").replace("contract = job", "contract = nope");
    assert_eq!(
        errors(&text),
        ["line 4: contract is an unknown contract: nope"]
    );
}

#[test]
fn ice_belongs_to_its_host() {
    let text =
        format!("{NET}[ice b/sentry]\ntype = sentry\n[ice b/hound]\ntype = hound\nversion = 2\n");
    let content = Content::parse(&text).unwrap();
    let ice = |name: &str, kind, version| IceDef {
        name: name.to_owned(),
        kind,
        version,
        guards: None,
    };
    assert_eq!(
        content.hosts[1].ice,
        [
            ice("sentry", IceKind::Sentry, 1),
            ice("hound", IceKind::Hound, 2)
        ]
    );
    assert_eq!(content.hosts[2].ice, []);
}

#[test]
fn a_warden_guards_a_route() {
    let text = format!("{NET}[ice b/warden]\ntype = warden\nguards = c\n");
    let content = Content::parse(&text).unwrap();
    assert_eq!(content.hosts[1].ice[0].guards.as_deref(), Some("c"));
}

#[test]
fn bad_guards_fail() {
    let text = format!(
        "{NET}\
[ice b/warden]
type = warden
[ice c/gate]
type = warden
guards = a
[ice b/scorch]
type = scorch
guards = c
"
    );
    assert_eq!(
        errors(&text),
        [
            "line 19: ice b/warden has no guards",
            "line 21: c/gate guards no route to a",
            "line 26: only a warden can guard a route",
        ]
    );
}

#[test]
fn bad_ice_fails() {
    let text = format!(
        "{NET}\
[ice a/sentry]
type = sentry
[ice b/x]
type = dragon
[ice b/hound]
type = hound
version = 10
[ice z/sentry]
type = sentry
[ice c/twin]
type = sentry
[ice c/twin]
type = scorch
[ice c/blank]
[ice nohost]
type = sentry
"
    );
    assert_eq!(
        errors(&text),
        [
            "line 19: ice must be on a host in a network: a/sentry",
            "line 22: unknown ICE type: dragon (use sentry, warden, hound, or scorch)",
            "line 25: bad ICE version: 10 (use 1 to 9)",
            "line 26: ice is on an unknown host: z",
            "line 30: ice c/twin is defined more than once",
            "line 32: ice c/blank has no type",
            "line 33: ice needs a host and a name, for example [ice gate/sentry]",
        ]
    );
}

const CITY: &str = "\
# A small city.
[city test]
name = Test City
start = a

[host a]
name = Host A
desc = The first host.
route = b

[host b]
name = Host B
desc = The second host.

[file a/notes.txt]
line = One.
line = Two.
";

fn errors(text: &str) -> Vec<String> {
    match Content::parse(text) {
        Ok(_) => Vec::new(),
        Err(errors) => errors.iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn reads_the_city_and_its_hosts() {
    let content = Content::parse(CITY).unwrap();
    assert_eq!(content.city, "Test City");
    assert_eq!(content.start, "a");
    let ids: Vec<&str> = content.hosts.iter().map(|host| host.id.as_str()).collect();
    assert_eq!(ids, ["a", "b"]);
    assert_eq!(content.hosts[0].name, "Host A");
    assert_eq!(content.hosts[0].description, "The first host.");
    assert_eq!(content.hosts[0].routes, ["b"]);
}

#[test]
fn a_byte_order_mark_is_ignored() {
    assert_eq!(
        Content::parse(&format!("\u{feff}{CITY}")),
        Content::parse(CITY)
    );
}

#[test]
fn files_belong_to_their_host() {
    let content = Content::parse(CITY).unwrap();
    let notes = File::new("notes.txt", vec!["One.".to_owned(), "Two.".to_owned()]);
    assert_eq!(content.hosts[0].files, [notes]);
    assert_eq!(content.hosts[1].files, Vec::<File>::new());
}

#[test]
fn unreadable_lines_and_stray_keys_fail_with_line_numbers() {
    assert_eq!(
        errors(&format!("name = x\n{CITY}oops\n")),
        [
            "line 1: key before the first section",
            "line 19: cannot read this line: oops",
        ]
    );
}

#[test]
fn section_headers_need_a_type_and_a_name() {
    let found = errors("[city]\n");
    assert!(
        found.contains(
            &"line 1: section needs a type and a name, for example [host relay]".to_owned()
        )
    );
}

#[test]
fn unknown_sections_and_keys_fail() {
    let text = CITY.replace("desc = The first", "desk = The first") + "[zone z]\n";
    assert_eq!(
        errors(&text),
        [
            "line 6: host a has no desc",
            "line 8: unknown key in host a: desk",
            "line 18: unknown section type: zone",
        ]
    );
}

#[test]
fn missing_and_repeated_keys_fail() {
    let text = "[city t]\nname = T\nname = U\n[host a]\nname = A\n";
    assert_eq!(
        errors(text),
        [
            "line 1: city t has no start",
            "line 3: name is set more than once in city t",
            "line 4: host a has no desc",
        ]
    );
}

#[test]
fn bad_references_fail() {
    let text = "[city t]\nname = T\nstart = nowhere\n[host a]\nname = A\ndesc = A.\n\
                route = ghost\nroute = a\n[file b/x.txt]\nline = X.\n[file x.txt]\nline = X.\n";
    assert_eq!(
        errors(text),
        [
            "line 3: start is an unknown host: nowhere",
            "line 7: route to an unknown host: ghost",
            "line 8: host a has a route to itself",
            "line 9: file is on an unknown host: b",
            "line 11: file needs a host and a name, for example [file relay/motd.txt]",
        ]
    );
}

#[test]
fn duplicates_fail() {
    let text = "[city t]\nname = T\nstart = a\n[city u]\nname = U\nstart = a\n\
                [host a]\nname = A\ndesc = A.\n[host a]\nname = A\ndesc = A.\n\
                [file a/x.txt]\nline = X.\n[file a/x.txt]\nline = Y.\n";
    assert_eq!(
        errors(text),
        [
            "line 4: more than one city section",
            "line 10: host a is defined more than once",
            "line 15: file a/x.txt is defined more than once",
        ]
    );
}

#[test]
fn files_need_at_least_one_line() {
    let text = "[city t]\nname = T\nstart = a\n[host a]\nname = A\ndesc = A.\n[file a/x.txt]\n";
    assert_eq!(errors(text), ["line 7: file a/x.txt has no line"]);
}

#[test]
fn a_city_section_is_necessary() {
    assert_eq!(
        errors("[host a]\nname = A\ndesc = A.\n"),
        ["no city section"]
    );
}

#[test]
fn unreachable_hosts_fail() {
    let text = "[city t]\nname = T\nstart = a\n[host a]\nname = A\ndesc = A.\n\
                [host b]\nname = B\ndesc = B.\n";
    assert_eq!(
        errors(text),
        ["line 7: host b cannot be reached from the start host"]
    );
}

#[test]
fn a_route_connects_both_hosts() {
    let text = "[city t]\nname = T\nstart = a\n[host a]\nname = A\ndesc = A.\n\
                [host b]\nname = B\ndesc = B.\nroute = a\n";
    assert_eq!(errors(text), Vec::<String>::new());
}

#[test]
fn the_city_can_have_a_home_host() {
    let content = Content::parse(&CITY.replace("start = a", "start = a\nhome = b")).unwrap();
    assert_eq!(content.home.as_deref(), Some("b"));
    assert_eq!(Content::parse(CITY).unwrap().home, None);
}

#[test]
fn a_home_must_be_a_known_hub_host() {
    let city = "[city t]\nname = T\nstart = a\nhome = HOME\n[host a]\nname = A\ndesc = A.\n\
                [network n]\nname = N\n[host b]\nname = B\ndesc = B.\nnetwork = n\nroute = a\n";
    assert_eq!(
        errors(&city.replace("HOME", "x")),
        ["line 4: home is an unknown host: x"]
    );
    assert_eq!(
        errors(&city.replace("HOME", "b")),
        ["line 4: home is in a network: b"]
    );
}

#[test]
fn hosts_can_be_in_a_network() {
    let text = format!(
        "{CITY}[network lab]\nname = The Lab\n[host c]\nname = C\ndesc = C.\nnetwork = lab\nroute = b\n"
    );
    let content = Content::parse(&text).unwrap();
    let lab = NetworkDef {
        id: "lab".to_owned(),
        name: "The Lab".to_owned(),
        security: Security::Low,
    };
    assert_eq!(content.networks, [lab]);
    assert_eq!(content.hosts[2].network.as_deref(), Some("lab"));
    assert_eq!(content.hosts[0].network, None);
}

#[test]
fn bad_networks_fail() {
    let text = "\
[city t]
name = T
start = a
[host a]
name = A
desc = A.
[network n]
name = N
[network m]
name = M
[network e]
name = E
[host b]
name = B
desc = B.
network = n
route = a
[host c]
name = C
desc = C.
network = m
route = b
[host d]
name = D
desc = D.
network = x
route = a
";
    assert_eq!(
        errors(text),
        [
            "line 11: network e has no hosts",
            "line 22: route between two networks: c and b",
            "line 26: host d is in an unknown network: x",
        ]
    );
}

#[test]
fn bad_names_fail() {
    let text = "[city t]\nname = T\nstart = a\n[host a]\nname = A\ndesc = A.\n\
                [host B!]\nname = B\ndesc = B.\nroute = a\n[file a/No.txt]\nline = X.\n";
    assert_eq!(
        errors(text),
        [
            "line 7: bad name: B! (use a-z, 0-9, and -)",
            "line 11: bad name: No.txt (use a-z, 0-9, -, and .)",
        ]
    );
}
