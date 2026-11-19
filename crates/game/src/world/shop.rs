use ttymud_text::{Message, Style};

use super::{HostId, Player, PlayerId, Step, World, lines, listed};
use crate::commands::{self, Verb};

#[derive(Debug)]
pub(super) struct Shop {
    pub(super) host: HostId,
    pub(super) programs: Vec<(Verb, u32)>,
    pub(super) ram: Vec<(u32, u32)>,
    pub(super) cpu: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, Copy)]
enum Ware {
    Program(Verb),
    Ram(u32),
    Cpu(u32),
}

impl Shop {
    fn wares(&self, state: &Player) -> Vec<(Ware, u32)> {
        let programs = self
            .programs
            .iter()
            .filter(|(verb, _)| !state.know.contains(verb))
            .map(|&(verb, price)| (Ware::Program(verb), price));
        let ram = self
            .ram
            .iter()
            .find(|&&(size, _)| size > state.rig.ram)
            .map(|&(size, price)| (Ware::Ram(size), price));
        let cpu = self
            .cpu
            .iter()
            .find(|&&(cycles, _)| cycles > state.rig.cpu)
            .map(|&(cycles, price)| (Ware::Cpu(cycles), price));
        programs.chain(ram).chain(cpu).collect()
    }
}

impl Ware {
    fn name(self) -> &'static str {
        match self {
            Self::Program(verb) => commands::name_of(verb),
            Self::Ram(_) => "ram",
            Self::Cpu(_) => "cpu",
        }
    }

    fn describe(self, state: &Player) -> String {
        match self {
            Self::Program(verb) => commands::summary_of(verb).to_owned(),
            Self::Ram(size) => format!("{size} GB of RAM, not {} GB.", state.rig.ram),
            Self::Cpu(cycles) => format!("{cycles} cycles, not {}.", state.rig.cpu),
        }
    }
}

impl World {
    pub(super) fn is_shop(&self, host: HostId) -> bool {
        self.shops.iter().any(|shop| shop.host == host)
    }

    fn shop_here(&self, player: PlayerId) -> Result<&Shop, Message> {
        let here = self.here(player);
        if let Some(shop) = self.shops.iter().find(|shop| shop.host == here) {
            return Ok(shop);
        }
        let hosts = self
            .shops
            .iter()
            .map(|shop| self.hosts[shop.host.0].id.as_str());
        let message = Message::error("There is no shop here. Shops: ");
        Err(listed(message, Style::Host, hosts))
    }

    pub(super) fn shop(&self, player: PlayerId) -> Result<Step, Message> {
        let shop = self.shop_here(player)?;
        let Some(state) = self.players.get(&player) else {
            return Ok(Step::default());
        };
        let wares = shop.wares(state);
        let mut shown = Vec::new();
        if wares.is_empty() {
            shown.push(Message::game("You have all that this shop sells."));
        } else {
            shown.push(Message::game("The shop sells:"));
        }
        for (ware, price) in wares {
            let rest = format!(": {price} cr. {}", ware.describe(state));
            shown.push(
                Message::game("")
                    .span(Style::Command, ware.name())
                    .text(rest),
            );
        }
        shown.push(Message::game(format!("Credits: {} cr", state.credits)));
        Ok(lines(shown))
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{plain, run, world};
    use crate::{PlayerId, World};

    fn shopper(kit: &str) -> (World, PlayerId) {
        let mut world = world();
        let (player, _) = world.connect("a");
        for line in ["nyx", kit, "go market"] {
            world.command(player, line);
        }
        (world, player)
    }

    #[test]
    fn the_shop_lists_what_the_handle_can_buy() {
        let (mut world, player) = shopper("ghost");
        assert_eq!(
            plain(&world.command(player, "shop")),
            [
                "The shop sells:",
                "patch: 300 cr. Repair some integrity during a run.",
                "firewall: 400 cr. Stop the next attack on your integrity.",
                "ram: 900 cr. 12 GB of RAM, not 8 GB.",
                "cpu: 1200 cr. 5 cycles, not 4.",
                "Credits: 0 cr",
            ]
        );
    }

    #[test]
    fn a_handle_with_all_wares_hears_so() {
        let (mut world, player) = shopper("breaker");
        if let Some(state) = world.players.get_mut(&player) {
            state.know.push(crate::commands::Verb::Mask);
            state.rig.ram = 16;
            state.rig.cpu = 5;
        }
        assert_eq!(
            plain(&world.command(player, "shop")),
            ["You have all that this shop sells.", "Credits: 0 cr"]
        );
    }

    #[test]
    fn shop_works_only_at_a_shop() {
        assert_eq!(run("shop"), ["There is no shop here. Shops: market"]);
    }
}
