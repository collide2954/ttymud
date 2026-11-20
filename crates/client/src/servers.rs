use ttymud_net::PublicKey;
use ttymud_text::ini::{self, Error, Section};
use ttymud_text::{is_address, is_server_name};

const LIST: &str = include_str!("../../../servers.ini");
const KEYS: [&str; 4] = ["city", "address", "key", "desc"];
const MAX_CITY: usize = 40;
const MAX_DESCRIPTION: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub name: String,
    pub city: String,
    pub address: String,
    pub key: PublicKey,
    pub description: String,
}

#[must_use]
pub fn servers() -> Vec<Server> {
    parse_servers(LIST).unwrap_or_default()
}

fn parse_servers(text: &str) -> Result<Vec<Server>, Vec<Error>> {
    let (sections, mut errors) = ini::read(text);
    let mut servers: Vec<Server> = Vec::new();
    for section in &sections {
        match read_server(section) {
            Ok(server) if servers.iter().any(|known| known.name == server.name) => {
                let message = format!("server {} is in the list more than once", server.name);
                errors.push(at(section.line, message));
            }
            Ok(server) => servers.push(server),
            Err(error) => errors.push(error),
        }
    }
    servers.sort_by(|a, b| a.name.cmp(&b.name));
    if errors.is_empty() {
        Ok(servers)
    } else {
        Err(errors)
    }
}

fn read_server(section: &Section<'_>) -> Result<Server, Error> {
    let (name, line) = (section.id, section.line);
    if section.kind != "server" {
        let kind = section.kind;
        return Err(at(
            line,
            format!("unknown section type: {kind} (use server)"),
        ));
    }
    if !is_server_name(name) {
        return Err(at(
            line,
            format!("bad server name: {name} (use 3 to 16 of a-z, 0-9, and -, but not localhost)"),
        ));
    }
    if let Some(error) = section.unknown(&KEYS) {
        return Err(error);
    }
    let (city, address, key, description) = (
        section.single("city")?,
        section.single("address")?,
        section.single("key")?,
        section.single("desc")?,
    );
    for (pair, limit) in [(city, MAX_CITY), (description, MAX_DESCRIPTION)] {
        let (text, field) = (pair.value, pair.key);
        if text.chars().count() > limit {
            let message = format!("the {field} of server {name} is longer than {limit} characters");
            return Err(at(pair.line, message));
        }
        if ttymud_text::clean(text) != text {
            let message = format!("the {field} of server {name} has a hidden or control character");
            return Err(at(pair.line, message));
        }
    }
    if !is_address(address.value) {
        let message = format!("bad address for server {name}: {}", address.value);
        return Err(at(address.line, message));
    }
    let Some(public) = PublicKey::from_hex(key.value) else {
        let message = format!("bad key for server {name} (use the Key line of ttymud-server init)");
        return Err(at(key.line, message));
    };
    Ok(Server {
        name: name.to_owned(),
        city: city.value.to_owned(),
        address: address.value.to_owned(),
        key: public,
        description: description.value.to_owned(),
    })
}

fn at(line: usize, message: String) -> Error {
    Error { line, message }
}

#[cfg(test)]
mod tests {
    use ttymud_net::PublicKey;

    use super::{LIST, Server, parse_servers};

    fn list(text: &str) -> Result<Vec<Server>, Vec<String>> {
        parse_servers(text).map_err(|errors| errors.iter().map(ToString::to_string).collect())
    }

    fn entry(name: &str) -> String {
        let key = "11".repeat(32);
        format!(
            "[server {name}]\ncity = Port Ostra\naddress = ostra.example.net\nkey = {key}\ndesc = The first city.\n"
        )
    }

    #[test]
    fn the_built_in_list_is_valid() {
        assert!(list(LIST).is_ok(), "{:?}", list(LIST));
    }

    #[test]
    fn a_server_has_a_name_a_city_an_address_a_key_and_a_line() {
        let ostra = Server {
            name: "ostra".to_owned(),
            city: "Port Ostra".to_owned(),
            address: "ostra.example.net".to_owned(),
            key: PublicKey::from_hex(&"11".repeat(32)).unwrap(),
            description: "The first city.".to_owned(),
        };
        assert_eq!(
            list(&format!("# A note.\n{}", entry("ostra"))),
            Ok(vec![ostra])
        );
        assert_eq!(list(""), Ok(Vec::new()));
    }

    #[test]
    fn the_list_is_in_the_order_of_the_names() {
        let names: Vec<String> = list(&(entry("zeta") + &entry("alpha") + &entry("mid")))
            .unwrap()
            .into_iter()
            .map(|server| server.name)
            .collect();
        assert_eq!(names, ["alpha", "mid", "zeta"]);
    }

    #[test]
    fn bad_entries_fail_with_a_line_and_a_reason() {
        let key = "11".repeat(32);
        let name_rule = "(use 3 to 16 of a-z, 0-9, and -, but not localhost)";
        let cases = [
            (
                "[host ostra]\n".to_owned(),
                "line 1: unknown section type: host (use server)".to_owned(),
            ),
            (
                entry("Big.Name"),
                format!("line 1: bad server name: Big.Name {name_rule}"),
            ),
            (
                entry("localhost"),
                format!("line 1: bad server name: localhost {name_rule}"),
            ),
            (
                entry("dock") + "port = 7331\n",
                "line 6: unknown key in server dock: port".to_owned(),
            ),
            (
                entry("dock").replace("desc = The first city.\n", ""),
                "line 1: server dock has no desc".to_owned(),
            ),
            (
                entry("dock") + "city = Again\n",
                "line 6: server dock has more than one city".to_owned(),
            ),
            (
                entry("dock").replace("The first city.", &"x".repeat(201)),
                "line 5: the desc of server dock is longer than 200 characters".to_owned(),
            ),
            (
                entry("dock").replace("Port Ostra", "Port\u{202e}Ostra"),
                "line 2: the city of server dock has a hidden or control character".to_owned(),
            ),
            (
                entry("dock").replace("ostra.example.net", "two words"),
                "line 3: bad address for server dock: two words".to_owned(),
            ),
            (
                entry("dock").replace(&key, "1234"),
                "line 4: bad key for server dock (use the Key line of ttymud-server init)"
                    .to_owned(),
            ),
            (
                entry("dock") + &entry("dock"),
                "line 6: server dock is in the list more than once".to_owned(),
            ),
        ];
        for (text, error) in cases {
            assert_eq!(list(&text), Err(vec![error]), "{text}");
        }
    }
}
