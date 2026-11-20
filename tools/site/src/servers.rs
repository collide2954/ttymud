use std::fmt::Write;

use ttymud::Server;

use crate::markdown::escape;

pub(crate) fn list(servers: &[Server]) -> String {
    if servers.is_empty() {
        return "<p class=\"card\">The list has no servers at this time. \
                <a href=\"#add\">Add your server</a> to the list.</p>\n"
            .to_owned();
    }
    let mut html = String::from("<ul class=\"servers\">\n");
    for server in servers {
        let name = escape(&server.name);
        let _ = writeln!(
            html,
            "<li class=\"card\">\n<h2 id=\"server-{name}\">{name}</h2>\n<p>{}</p>\n<dl>\n\
             <dt>City</dt><dd>{}</dd>\n\
             <dt>Address</dt><dd><code>{}</code></dd>\n\
             <dt>Key</dt><dd><code>{}</code></dd>\n</dl>\n\
             <pre class=\"code\"><code>ttymud connect {name}</code></pre>\n</li>",
            escape(&server.description),
            escape(&server.city),
            escape(&server.address),
            server.key.to_hex(),
        );
    }
    html.push_str("</ul>\n");
    html
}

#[cfg(test)]
mod tests {
    use ttymud::Server;
    use ttymud_net::PublicKey;

    use super::list;

    #[test]
    fn each_server_gets_a_card_with_its_details_and_the_command() {
        let key = "ab".repeat(32);
        let server = Server {
            name: "ostra".to_owned(),
            city: "Port Ostra".to_owned(),
            address: "ostra.example.net:7331".to_owned(),
            key: PublicKey::from_hex(&key).unwrap(),
            description: "Jobs & <b>heists</b>.".to_owned(),
        };
        assert_eq!(
            list(&[server]),
            format!(
                "<ul class=\"servers\">\n<li class=\"card\">\n\
                 <h2 id=\"server-ostra\">ostra</h2>\n<p>Jobs &amp; &lt;b&gt;heists&lt;/b&gt;.</p>\n<dl>\n\
                 <dt>City</dt><dd>Port Ostra</dd>\n\
                 <dt>Address</dt><dd><code>ostra.example.net:7331</code></dd>\n\
                 <dt>Key</dt><dd><code>{key}</code></dd>\n</dl>\n\
                 <pre class=\"code\"><code>ttymud connect ostra</code></pre>\n</li>\n</ul>\n"
            )
        );
    }

    #[test]
    fn an_empty_list_tells_how_to_add_a_server() {
        assert!(list(&[]).contains("<a href=\"#add\">Add your server</a>"));
    }
}
