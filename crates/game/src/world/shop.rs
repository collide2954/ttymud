use ttymud_text::{Message, Style};

use super::{HostId, Player, PlayerId, Step, Thing, World, choose, lines, listed, usage};
use crate::commands::{self, Item, Verb};
use crate::content::{File, ShopDef};

#[derive(Debug)]
pub(super) struct Shop {
    pub(super) host: HostId,
    pub(super) programs: Vec<(Verb, u32)>,
    pub(super) ram: Vec<(u32, u32)>,
    pub(super) cpu: Vec<(u32, u32)>,
}

pub(super) fn shops(defs: Vec<ShopDef>, find: impl Fn(&str) -> HostId) -> Vec<Shop> {
    defs.into_iter()
        .map(|def| Shop {
            host: find(&def.host),
            programs: def.programs,
            ram: def.ram,
            cpu: def.cpu,
        })
        .collect()
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

    pub(super) fn ware_names(&self, player: PlayerId) -> Vec<&'static str> {
        let (Ok(shop), Some(state)) = (self.shop_here(player), self.players.get(&player)) else {
            return Vec::new();
        };
        shop.wares(state)
            .into_iter()
            .map(|(ware, _)| ware.name())
            .collect()
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
        shown.push(
            Message::game("The fence buys stolen data. Type ")
                .span(Style::Command, "sell <file>")
                .text("."),
        );
        shown.push(Message::game(format!("Credits: {} cr", state.credits)));
        Ok(lines(shown))
    }

    pub(super) fn sell(
        &mut self,
        player: PlayerId,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        self.shop_here(player)?;
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        let names: Vec<String> = match (args, input) {
            ([], None) => return Err(wanted(&state.rig.files)),
            ([], Some(items)) => items
                .into_iter()
                .filter_map(|item| match item {
                    Item::File(file) => Some(file.name),
                    Item::Route(_) | Item::Ice { .. } | Item::Line(_) => None,
                })
                .collect(),
            (_, Some(_)) => return Err(Message::error("sell takes a file or a pipe, not both.")),
            (args, None) => {
                let carried: Vec<&str> = state
                    .rig
                    .files
                    .iter()
                    .map(|file| file.name.as_str())
                    .collect();
                let chosen: Result<Vec<&str>, Message> = args
                    .iter()
                    .map(|arg| choose(Thing::File, arg, &carried))
                    .collect();
                chosen?.into_iter().map(str::to_owned).collect()
            }
        };
        let mut shown = Vec::new();
        for name in names {
            let index = state.rig.files.iter().position(|file| file.name == name);
            let Some(index) = index else {
                shown.push(Message::game(format!("{name} is not on your rig.")));
                continue;
            };
            let value = state.rig.files[index].value;
            if value == 0 {
                shown.push(Message::game(format!("The fence does not want {name}.")));
                continue;
            }
            if state.sold.contains(&name) {
                shown.push(Message::game(format!(
                    "The fence already bought {name} from you in this job."
                )));
                continue;
            }
            state.rig.files.remove(index);
            state.credits += value;
            state.sold.push(name.clone());
            shown.push(
                Message::game(format!("The fence pays you {value} cr for "))
                    .span(Style::File, &name)
                    .text("."),
            );
        }
        Ok(Step::show(shown))
    }

    pub(super) fn buy(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let shop = self.shop_here(player)?;
        let Some(state) = self.players.get(&player) else {
            return Ok(Step::default());
        };
        let wares = shop.wares(state);
        let names: Vec<&str> = wares.iter().map(|(ware, _)| ware.name()).collect();
        let [word] = args else {
            return Err(usage("buy <item>", Thing::Ware, &names));
        };
        let word = word.to_lowercase();
        let known = shop
            .programs
            .iter()
            .any(|&(verb, _)| commands::name_of(verb) == word && state.know.contains(&verb));
        let refusal = match word.as_str() {
            _ if known => Some(format!("You already know {word}.")),
            "ram" if !names.contains(&"ram") => {
                Some("Your rig has all the RAM that this shop sells.".to_owned())
            }
            "cpu" if !names.contains(&"cpu") => {
                Some("Your rig has the fastest CPU that this shop sells.".to_owned())
            }
            _ => None,
        };
        if let Some(refusal) = refusal {
            return Err(Message::error(refusal));
        }
        let name = choose(Thing::Ware, &word, &names)?;
        let index = names.iter().position(|&known| known == name).unwrap_or(0);
        let (ware, price) = wares[index];
        if state.credits < price {
            let credits = state.credits;
            return Err(Message::error(format!(
                "{name} costs {price} cr. You have {credits} cr."
            )));
        }
        let home = self.home.map(|home| self.hosts[home.0].id.clone());
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        state.credits -= price;
        let bought = format!("You buy {name} for {price} cr.");
        let message = match ware {
            Ware::Program(verb) => {
                state.know.push(verb);
                match home {
                    Some(home) => format!("{bought} Load it at {home}."),
                    None => bought,
                }
            }
            Ware::Ram(size) => {
                state.rig.ram = size;
                let ram = state.rig.ram_line();
                format!("You buy {size} GB of RAM for {price} cr. {ram}.")
            }
            Ware::Cpu(cycles) => {
                state.rig.cpu = cycles;
                format!("You buy a faster CPU for {price} cr. It holds {cycles} cycles.")
            }
        };
        Ok(Step::show(vec![Message::game(message)]))
    }
}

fn wanted(files: &[File]) -> Message {
    let mut message = Message::error("Usage: sell <file>. The fence wants: ");
    let valued: Vec<&File> = files.iter().filter(|file| file.value > 0).collect();
    if valued.is_empty() {
        return message.text("nothing on your rig");
    }
    for (index, file) in valued.iter().enumerate() {
        if index > 0 {
            message = message.text(", ");
        }
        message = message
            .span(Style::File, &file.name)
            .text(format!(" ({} cr)", file.value));
    }
    message
}

#[cfg(test)]
mod tests {
    use crate::content::File;
    use crate::testing::{hired_world, plain, run, world};
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
                "The fence buys stolen data. Type sell <file>.",
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
            [
                "You have all that this shop sells.",
                "The fence buys stolen data. Type sell <file>.",
                "Credits: 0 cr"
            ]
        );
    }

    #[test]
    fn shop_works_only_at_a_shop() {
        assert_eq!(run("shop"), ["There is no shop here. Shops: market"]);
        assert_eq!(run("buy ram"), ["There is no shop here. Shops: market"]);
    }

    fn rich(kit: &str, credits: u32) -> (World, PlayerId) {
        let (mut world, player) = shopper(kit);
        if let Some(state) = world.players.get_mut(&player) {
            state.credits = credits;
        }
        (world, player)
    }

    #[test]
    fn a_bought_program_joins_the_know_how() {
        let (mut world, player) = rich("ghost", 500);
        assert_eq!(
            plain(&world.command(player, "buy firewall")),
            ["You buy firewall for 400 cr. Load it at back-room."]
        );
        for line in ["back", "go back-room", "unload scrub"] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "load firewall")),
            ["You load firewall. RAM: 8/8 GB."]
        );
        assert_eq!(
            plain(&world.command(player, "status | grep credits")),
            ["Credits: 100 cr"]
        );
    }

    #[test]
    fn hardware_makes_the_rig_better() {
        let (mut world, player) = rich("ghost", 2100);
        assert_eq!(
            plain(&world.command(player, "buy ram")),
            ["You buy 12 GB of RAM for 900 cr. RAM: 8/12 GB."]
        );
        assert_eq!(
            plain(&world.command(player, "buy cpu")),
            ["You buy a faster CPU for 1200 cr. It holds 5 cycles."]
        );
        assert_eq!(
            plain(&world.command(player, "shop | grep ram")),
            ["ram: 1800 cr. 16 GB of RAM, not 12 GB."]
        );
    }

    fn carry(world: &mut World, player: PlayerId, name: &str, value: u32) {
        let mut file = File::new(name, vec!["Data.".to_owned()]);
        file.value = value;
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.files.push(file);
        }
    }

    #[test]
    fn the_fence_buys_stolen_data() {
        let (mut world, player) = shopper("ghost");
        carry(&mut world, player, "payroll.db", 40);
        assert_eq!(
            plain(&world.command(player, "sell payroll.db")),
            ["The fence pays you 40 cr for payroll.db."]
        );
        assert_eq!(
            plain(&world.command(player, "inventory")),
            ["Your rig has no files."]
        );
        assert_eq!(
            plain(&world.command(player, "status | grep credits")),
            ["Credits: 40 cr"]
        );
    }

    #[test]
    fn the_fence_wants_only_files_with_a_value() {
        let (mut world, player) = shopper("ghost");
        carry(&mut world, player, "memo.txt", 0);
        carry(&mut world, player, "clients.db", 100);
        assert_eq!(
            plain(&world.command(player, "sell memo.txt")),
            ["The fence does not want memo.txt."]
        );
        assert_eq!(
            plain(&world.command(player, "sell")),
            ["Usage: sell <file>. The fence wants: clients.db (100 cr)"]
        );
        assert_eq!(
            plain(&world.command(player, "inventory | sell")),
            [
                "The fence does not want memo.txt.",
                "The fence pays you 100 cr for clients.db."
            ]
        );
    }

    #[test]
    fn the_fence_buys_a_file_one_time_in_each_job() {
        let mut world = hired_world();
        let (player, _) = world.connect("a");
        for line in ["nyx", "ghost", "go market"] {
            world.command(player, line);
        }
        carry(&mut world, player, "payroll.db", 40);
        world.command(player, "sell payroll.db");
        carry(&mut world, player, "payroll.db", 40);
        assert_eq!(
            plain(&world.command(player, "sell payroll.db")),
            ["The fence already bought payroll.db from you in this job."]
        );
        world.command(player, "contract drop");
        assert_eq!(
            plain(&world.command(player, "sell payroll.db")),
            ["The fence pays you 40 cr for payroll.db."]
        );
    }

    #[test]
    fn the_shop_names_the_fence() {
        let (mut world, player) = shopper("ghost");
        assert_eq!(
            plain(&world.command(player, "shop | grep fence")),
            ["The fence buys stolen data. Type sell <file>."]
        );
    }

    #[test]
    fn buy_needs_enough_credits() {
        let (mut world, player) = rich("ghost", 399);
        assert_eq!(
            plain(&world.command(player, "buy firewall")),
            ["firewall costs 400 cr. You have 399 cr."]
        );
    }

    #[test]
    fn buy_names_what_the_shop_sells() {
        let (mut world, player) = rich("ghost", 0);
        let wares = "patch, firewall, ram, cpu";
        assert_eq!(
            plain(&world.command(player, "buy nuke")),
            [format!("The shop does not sell nuke. For sale: {wares}")]
        );
        assert_eq!(
            plain(&world.command(player, "buy")),
            [format!("Usage: buy <item>. For sale: {wares}")]
        );
        assert_eq!(
            plain(&world.command(player, "buy mask")),
            ["You already know mask."]
        );
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.ram = 16;
        }
        assert_eq!(
            plain(&world.command(player, "buy ram")),
            ["Your rig has all the RAM that this shop sells."]
        );
    }
}
