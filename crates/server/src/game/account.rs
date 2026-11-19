use ttymud_net::{PublicKey, digest, random_code};
use ttymud_text::{Message, Style, sanitize};

use super::{Game, PlayerId};
use crate::city::now;

const RECOVERY_BYTES: usize = 16;
const LINK_BYTES: usize = 8;
const NO_CODE: &str = "The server cannot make a code now. Try again later.";

impl Game {
    pub(super) fn account_command(&mut self, player: PlayerId, line: &str) -> Option<PlayerId> {
        let line = sanitize(line);
        let words: Vec<&str> = line.split_whitespace().collect();
        let (first, args) = words.split_first()?;
        let name = first.to_ascii_lowercase();
        if self.world.handle(player).is_none() {
            let [code] = args else {
                return None;
            };
            return match name.as_str() {
                "link" => Some(self.use_link(player, code)),
                "recover" => Some(self.use_recovery(player, code)),
                _ => None,
            };
        }
        match (name.as_str(), args) {
            ("recovery", []) => self.new_recovery(player, true),
            ("link", []) => self.new_link(player),
            ("keys", []) => self.show_keys(player),
            ("revoke", [number]) => self.revoke(player, number),
            ("revoke", _) => self.show(player, vec![Message::error("Usage: revoke <number>.")]),
            ("recovery" | "link" | "keys", _) => {
                let refusal = format!("{name} takes no arguments.");
                self.show(player, vec![Message::error(refusal)]);
            }
            _ => return None,
        }
        Some(player)
    }

    fn account(&self, player: PlayerId) -> Option<(String, PublicKey)> {
        let link = self.links.get(&player)?;
        Some((link.account.clone(), link.key))
    }

    pub(super) fn first_recovery(&mut self, player: PlayerId) {
        let account = self.account(player);
        if account.is_some_and(|(account, _)| !self.identity.has_recovery(&account)) {
            self.new_recovery(player, false);
        }
    }

    fn new_recovery(&mut self, player: PlayerId, again: bool) {
        let Some((account, _)) = self.account(player) else {
            return;
        };
        let Ok((code, hash)) = code_and_digest(RECOVERY_BYTES) else {
            self.show(player, vec![Message::error(NO_CODE)]);
            return;
        };
        self.identity.set_recovery(&account, hash);
        let mut shown = vec![Message::game("Your recovery code: ").span(Style::Command, code)];
        if again {
            shown.push(Message::game("Your old recovery code does not work now."));
        }
        shown.push(Message::game(
            "Write it down and keep it safe. The server shows it only one time.",
        ));
        shown.push(Message::game(
            "On a new device, type recover <code> at the handle prompt to get your account back.",
        ));
        self.show(player, shown);
    }

    fn use_recovery(&mut self, player: PlayerId, code: &str) -> PlayerId {
        let Some((_, key)) = self.account(player) else {
            return player;
        };
        let found = digest(code)
            .ok()
            .and_then(|hash| self.identity.recover(&hash, &key.to_hex()));
        let Some(account) = found else {
            let refusal = Message::error("That recovery code does not work.");
            self.show(player, vec![refusal]);
            return player;
        };
        self.show(player, vec![Message::game("You recovered your account.")]);
        let player = self.switch(player, account);
        self.new_recovery(player, true);
        self.show(
            player,
            vec![Message::game(
                "Type keys to see the keys of your account. Type revoke <number> to remove a key that you lost.",
            )],
        );
        player
    }

    fn new_link(&mut self, player: PlayerId) {
        let Some((account, _)) = self.account(player) else {
            return;
        };
        let Ok((code, hash)) = code_and_digest(LINK_BYTES) else {
            self.show(player, vec![Message::error(NO_CODE)]);
            return;
        };
        self.identity.offer(&account, hash, now());
        let shown = vec![
            Message::game("Your link code: ").span(Style::Command, code),
            Message::game(
                "On your new device, type link <code> at the handle prompt. The code works one time, for 10 minutes.",
            ),
        ];
        self.show(player, shown);
    }

    fn use_link(&mut self, player: PlayerId, code: &str) -> PlayerId {
        let Some((_, key)) = self.account(player) else {
            return player;
        };
        let found = digest(code)
            .ok()
            .and_then(|hash| self.identity.accept(&hash, &key.to_hex(), now()));
        let Some(account) = found else {
            let refusal = Message::error(
                "That link code does not work. Type link on your other device to make a new code.",
            );
            self.show(player, vec![refusal]);
            return player;
        };
        let linked = Message::game("This device is now linked to your account.");
        self.show(player, vec![linked]);
        self.switch(player, account)
    }

    fn switch(&mut self, player: PlayerId, account: String) -> PlayerId {
        let Some(mut link) = self.links.remove(&player) else {
            return player;
        };
        self.connections.remove(&link.connection);
        let outputs = self.world.leave(player);
        self.deliver(outputs, None);
        link.account = account;
        link.status = None;
        self.start(link)
    }

    fn show_keys(&self, player: PlayerId) {
        let Some((account, key)) = self.account(player) else {
            return;
        };
        let current = key.to_hex();
        let mut shown = vec![Message::game("Keys of your account:")];
        for (index, known) in self.identity.keys(&account).iter().enumerate() {
            let mark = if *known == current {
                " (this device)"
            } else {
                ""
            };
            let number = index + 1;
            let print = fingerprint(known);
            shown.push(Message::game(format!("{number}. {print}{mark}")));
        }
        shown.push(Message::game("Type revoke <number> to remove a key."));
        self.show(player, shown);
    }

    fn revoke(&mut self, player: PlayerId, number: &str) {
        let Some((account, key)) = self.account(player) else {
            return;
        };
        let keys = self.identity.keys(&account);
        let chosen = number
            .parse::<usize>()
            .ok()
            .and_then(|index| Some((index, keys.get(index.checked_sub(1)?)?)));
        let Some((index, revoked)) = chosen else {
            let refusal = format!("No key {number}. Type keys to see the keys.");
            self.show(player, vec![Message::error(refusal)]);
            return;
        };
        if *revoked == key.to_hex() {
            let refusal = Message::error("You cannot revoke the key of this device.");
            self.show(player, vec![refusal]);
            return;
        }
        self.identity.revoke(&account, revoked);
        let print = fingerprint(revoked);
        let done = format!("You revoke key {index}, {print}. That device cannot connect now.");
        self.show(player, vec![Message::game(done)]);
    }
}

fn code_and_digest(bytes: usize) -> Result<(String, String), ttymud_net::Error> {
    let code = random_code(bytes)?;
    let hash = digest(&code)?;
    Ok((code, hash))
}

fn fingerprint(hex: &str) -> String {
    PublicKey::from_hex(hex).map_or_else(|| hex.to_owned(), |key| key.fingerprint())
}
