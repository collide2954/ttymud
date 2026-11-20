mod markdown;

use std::env;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use markdown::{Page, escape, render};

const REPOSITORY: &str = "https://github.com/collide2954/ttymud";
const GUIDES: [&str; 2] = ["player", "operator"];
const HOME_TITLE: &str = "TTYmud: a hacker game for the terminal";
const HOME_TEXT: &str = "TTYmud is a multiplayer hacker game for the terminal. Take contracts, hack into corporate networks, and stay ahead of the trace.";

fn main() -> ExitCode {
    let Some(out) = env::args_os().nth(1) else {
        eprintln!("Usage: site <output directory>");
        return ExitCode::FAILURE;
    };
    match build(&root(), Path::new(&out)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("site: {error}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn build(root: &Path, out: &Path) -> io::Result<()> {
    let site = root.join("site");
    let layout = fs::read_to_string(site.join("layout.html"))?;
    fs::create_dir_all(out)?;
    for entry in fs::read_dir(&site)? {
        let path = entry?.path();
        let Some(name) = path.file_name() else {
            continue;
        };
        if path.is_file() && name != "layout.html" && name != "home.html" {
            fs::copy(&path, out.join(name))?;
        }
    }
    let home = fs::read_to_string(site.join("home.html"))?;
    let index = fill(&layout, "index.html", HOME_TITLE, HOME_TEXT, &home);
    fs::write(out.join("index.html"), index)?;
    for name in GUIDES {
        let markdown = fs::read_to_string(root.join("docs").join(format!("{name}.md")))?;
        let page = render(&markdown, &target);
        let file = format!("{name}.html");
        let title = format!("{} · TTYmud", page.title);
        let html = fill(&layout, &file, &title, &page.summary, &guide(&page));
        fs::write(out.join(&file), html)?;
    }
    Ok(())
}

fn guide(page: &Page) -> String {
    let links = page
        .sections
        .iter()
        .fold(String::new(), |mut html, (id, title)| {
            let _ = writeln!(html, "<li><a href=\"#{id}\">{}</a></li>", escape(title));
            html
        });
    format!(
        "<div class=\"wrap guide\">\n\
         <nav class=\"contents\" aria-label=\"Contents\">\n<p>Contents</p>\n<ol>\n{links}</ol>\n</nav>\n\
         <details class=\"contents-small\">\n<summary>Contents</summary>\n<ol>\n{links}</ol>\n</details>\n\
         <article>\n{}</article>\n</div>\n",
        page.body
    )
}

fn fill(layout: &str, file: &str, title: &str, description: &str, content: &str) -> String {
    let link = format!("href=\"{file}\"");
    layout
        .replace("{{title}}", &escape(title))
        .replace("{{description}}", &escape(description))
        .replace(&link, &format!("{link} aria-current=\"page\""))
        .replace("{{content}}", content)
}

fn target(url: &str) -> String {
    if url.starts_with('#') || url.contains("://") {
        return url.to_owned();
    }
    let (path, anchor) = match url.split_once('#') {
        Some((path, anchor)) => (path, format!("#{anchor}")),
        None => (url, String::new()),
    };
    if let Some(name) = path
        .strip_suffix(".md")
        .filter(|name| GUIDES.contains(name))
    {
        return format!("{name}.html{anchor}");
    }
    let file = path
        .strip_prefix("../")
        .map_or_else(|| format!("docs/{path}"), str::to_owned);
    format!("{REPOSITORY}/blob/main/{file}{anchor}")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{REPOSITORY, build, fill, root, target};
    use crate::markdown::render;

    #[test]
    fn links_to_the_guides_stay_on_the_site_and_other_docs_go_to_github() {
        assert_eq!(target("player.md"), "player.html");
        assert_eq!(target("operator.md#game-data"), "operator.html#game-data");
        assert_eq!(
            target("design/gameplay.md#money"),
            format!("{REPOSITORY}/blob/main/docs/design/gameplay.md#money")
        );
        assert_eq!(
            target("../README.md"),
            format!("{REPOSITORY}/blob/main/README.md")
        );
        assert_eq!(target("https://rustup.rs"), "https://rustup.rs");
        assert_eq!(target("#heat"), "#heat");
    }

    #[test]
    fn the_layout_gets_the_title_the_description_and_the_content() {
        let layout = "<title>{{title}}</title><meta content=\"{{description}}\">\
                      <a href=\"index.html\">Home</a><a href=\"player.html\">Play</a>{{content}}";
        assert_eq!(
            fill(layout, "player.html", "A & B", "Say \"hi\"", "<p>Hi.</p>"),
            "<title>A &amp; B</title><meta content=\"Say &quot;hi&quot;\">\
             <a href=\"index.html\">Home</a><a href=\"player.html\" aria-current=\"page\">Play</a><p>Hi.</p>"
        );
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ttymud-site-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    #[test]
    fn build_makes_the_home_page_the_guides_and_copies_the_rest() {
        let root = scratch("root");
        let layout = "<title>{{title}}</title>|{{description}}|{{content}}";
        write(&root.join("site/layout.html"), layout);
        write(&root.join("site/home.html"), "<p>Home.</p>");
        write(&root.join("site/style.css"), "body {}");
        write(
            &root.join("docs/player.md"),
            "# Player guide\n\nHow to play.\n\n## Connect\n\nSee the [operator guide](operator.md).\n",
        );
        write(
            &root.join("docs/operator.md"),
            "# Operator guide\n\nHow to host.\n",
        );
        let out = scratch("out");
        build(&root, &out).unwrap();
        let read = |name: &str| fs::read_to_string(out.join(name)).unwrap();
        assert_eq!(read("style.css"), "body {}");
        assert!(
            read("index.html")
                .starts_with("<title>TTYmud: a hacker game for the terminal</title>|")
        );
        assert!(read("index.html").ends_with("|<p>Home.</p>"));
        let player = read("player.html");
        assert!(
            player.starts_with("<title>Player guide · TTYmud</title>|How to play.|"),
            "{player}"
        );
        assert!(
            player.contains("<a href=\"#connect\">Connect</a>"),
            "{player}"
        );
        assert!(
            player.contains("<a href=\"operator.html\">operator guide</a>"),
            "{player}"
        );
        assert!(read("operator.html").contains("<h1 id=\"operator-guide\">Operator guide</h1>"));
        assert!(!out.join("layout.html").exists() && !out.join("home.html").exists());
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn the_real_guides_render_without_raw_markdown() {
        for name in ["player.md", "operator.md"] {
            let markdown = fs::read_to_string(root().join("docs").join(name)).unwrap();
            let page = render(&markdown, &target);
            assert!(!page.title.is_empty(), "{name}");
            assert!(!page.sections.is_empty(), "{name}");
            for mark in ["**", "](", "| ---", "\n|", "\n#", "\n- ", "\n> "] {
                assert!(!page.body.contains(mark), "{name}: {mark}");
            }
        }
    }
}
