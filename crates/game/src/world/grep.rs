use ttymud_text::Message;

use super::{Step, listing};
use crate::commands::Item;

pub(super) fn grep(args: &[String], items: Vec<Item>) -> Result<Step, Message> {
    let [pattern] = args else {
        return Err(Message::error(
            "Usage: grep <pattern>. Example: ls | grep txt",
        ));
    };
    let needle = pattern.to_lowercase();
    let items: Vec<Item> = items
        .into_iter()
        .filter(|item| item.name().to_lowercase().contains(&needle))
        .collect();
    if items.is_empty() {
        return Err(Message::error(format!("No match for {pattern}.")));
    }
    Ok(Step {
        shown: listing(&items),
        items,
    })
}

#[cfg(test)]
mod tests {
    use crate::testing::run;

    #[test]
    fn grep_filters_piped_items() {
        assert_eq!(run("ls | grep txt"), ["motd.txt", "rules.txt"]);
    }

    #[test]
    fn grep_ignores_case() {
        assert_eq!(run("ls | grep MARKET"), ["market/"]);
    }

    #[test]
    fn look_pipes_the_same_items_as_ls() {
        assert_eq!(run("look | grep ba"), ["back-room/", "bazaar/"]);
    }

    #[test]
    fn grep_can_follow_grep() {
        assert_eq!(run("ls | grep a | grep ba"), ["back-room/", "bazaar/"]);
    }

    #[test]
    fn grep_without_a_match_fails() {
        assert_eq!(run("ls | grep zzz | read"), ["No match for zzz."]);
        let (mut world, player) = crate::testing::joined();
        let outputs = world.command(player, "ls | grep zzz");
        assert_eq!(outputs[0].message.kind, ttymud_text::Kind::Error);
    }

    #[test]
    fn grep_needs_input() {
        assert_eq!(
            run("grep txt"),
            ["grep needs input from a pipe. Example: ls | grep txt"]
        );
    }

    #[test]
    fn grep_needs_one_pattern() {
        assert_eq!(
            run("ls | grep"),
            ["Usage: grep <pattern>. Example: ls | grep txt"]
        );
    }
}
