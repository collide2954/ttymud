#[must_use]
pub fn is_server_name(name: &str) -> bool {
    (3..=16).contains(&name.len())
        && name != "localhost"
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[must_use]
pub fn is_address(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c))
}

#[cfg(test)]
mod tests {
    use super::{is_address, is_server_name};

    #[test]
    fn a_server_name_has_3_to_16_small_letters_digits_and_hyphens() {
        for good in ["ost", "night-owl", "dock-7", "abcdefghijklmnop"] {
            assert!(is_server_name(good), "{good}");
        }
        let bad = [
            "os",
            "abcdefghijklmnopq",
            "Ostra",
            "dock.7",
            "dock_7",
            "localhost",
            "naïve",
        ];
        for bad in bad {
            assert!(!is_server_name(bad), "{bad}");
        }
    }

    #[test]
    fn an_address_is_a_host_or_an_ip_address_with_a_port_or_without() {
        let good = [
            "ostra.example.net",
            "ostra.example.net:7400",
            "203.0.113.7",
            "[2001:db8::7]:7400",
        ];
        for good in good {
            assert!(is_address(good), "{good}");
        }
        for bad in ["", "two words", "ostra/x", "ostra\u{1b}[2J", "ostra.net\n"] {
            assert!(!is_address(bad), "{bad:?}");
        }
    }
}
