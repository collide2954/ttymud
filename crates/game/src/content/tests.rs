use super::{Content, ContractDef, File, IceDef, NetworkDef};
use crate::ice::IceKind;

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

fn with_contract(contract: &str) -> String {
    let city = NET.replace("start = a", "start = a\ncontract = job");
    format!("{city}[file c/x.db]\nline = X.\n{contract}")
}

#[test]
fn the_city_offers_a_contract() {
    let text = with_contract(
        "[contract job]\nfixer = Pell\nnetwork = n\ngoal = x.db\noffer = Get x.db.\noffer = Go.\n",
    );
    let content = Content::parse(&text).unwrap();
    let job = ContractDef {
        id: "job".to_owned(),
        fixer: "Pell".to_owned(),
        network: "n".to_owned(),
        goal: "x.db".to_owned(),
        offer: vec!["Get x.db.".to_owned(), "Go.".to_owned()],
    };
    assert_eq!(content.contracts, [job]);
    assert_eq!(content.contract.as_deref(), Some("job"));
}

#[test]
fn bad_contracts_fail() {
    let text = with_contract(
        "[contract job]\nfixer = Pell\nnetwork = n\ngoal = y.db\noffer = Get y.db.\n\
         [contract far]\nfixer = Pell\nnetwork = z\ngoal = x.db\noffer = Go.\n[contract bare]\n",
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
    let notes = File {
        name: "notes.txt".to_owned(),
        lines: vec!["One.".to_owned(), "Two.".to_owned()],
    };
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
