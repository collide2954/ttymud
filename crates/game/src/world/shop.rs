use ttymud_text::{Message, Style};

use super::builds::effect;
use super::market::{FULL, Market};
use super::{HostId, Player, PlayerId, Step, Thing, World, choose, lines, listed, usage};
use crate::commands::{self, Item, Verb};
use crate::content::{File, ShopDef, blueprint_name};

const DARKNET_PAY: u32 = 150;

#[derive(Debug)]
pub(super) struct Shop {
    pub(super) host: HostId,
    pub(super) programs: Vec<(Verb, u32)>,
    pub(super) blueprints: Vec<Blueprint>,
    pub(super) ram: Vec<(u32, u32)>,
    pub(super) cpu: Vec<(u32, u32)>,
    pub(super) fence: Option<Market>,
}

#[derive(Debug)]
pub(super) struct Blueprint {
    verb: Verb,
    version: u32,
    name: String,
    price: u32,
}

pub(super) fn shops(
    defs: Vec<ShopDef>,
    find: impl Fn(&str) -> HostId,
    darknet: impl Fn(HostId) -> bool,
) -> Vec<Shop> {
    defs.into_iter()
        .map(|def| {
            let host = find(&def.host);
            let blueprints = def
                .blueprints
                .into_iter()
                .map(|(verb, version, price)| Blueprint {
                    verb,
                    version,
                    name: blueprint_name(verb, version),
                    price,
                });
            Shop {
                host,
                programs: def.programs,
                blueprints: blueprints.collect(),
                ram: def.ram,
                cpu: def.cpu,
                fence: darknet(host).then(Market::default),
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
enum Ware {
    Program(Verb),
    Blueprint(Verb, u32),
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
        let blueprints = self
            .blueprints
            .iter()
            .filter(|plan| !state.blueprints.contains(&(plan.verb, plan.version)))
            .map(|plan| (Ware::Blueprint(plan.verb, plan.version), plan.price));
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
        programs.chain(blueprints).chain(ram).chain(cpu).collect()
    }

    fn refusal(&self, state: &Player, word: &str, names: &[&str]) -> Option<String> {
        let known = self
            .programs
            .iter()
            .any(|&(verb, _)| commands::name_of(verb) == word && state.know.contains(&verb));
        if known {
            return Some(format!("You already know {word}."));
        }
        let owned = self.blueprints.iter().find(|plan| {
            plan.name == word && state.blueprints.contains(&(plan.verb, plan.version))
        });
        if let Some(plan) = owned {
            let (name, version) = (commands::name_of(plan.verb), plan.version);
            return Some(format!(
                "You already have the blueprint for {name} v{version}."
            ));
        }
        match word {
            "ram" if !names.contains(&"ram") => {
                Some("Your rig has all the RAM that this shop sells.".to_owned())
            }
            "cpu" if !names.contains(&"cpu") => {
                Some("Your rig has the fastest CPU that this shop sells.".to_owned())
            }
            _ => None,
        }
    }
}

impl Ware {
    fn name(self, shop: &Shop) -> &str {
        match self {
            Self::Program(verb) => commands::name_of(verb),
            Self::Blueprint(verb, version) => shop
                .blueprints
                .iter()
                .find(|plan| plan.verb == verb && plan.version == version)
                .map_or("", |plan| plan.name.as_str()),
            Self::Ram(_) => "ram",
            Self::Cpu(_) => "cpu",
        }
    }

    fn describe(self, state: &Player) -> String {
        match self {
            Self::Program(verb) => commands::summary_of(verb).to_owned(),
            Self::Blueprint(verb, version) => {
                let (name, effect) = (commands::name_of(verb), effect(verb, version));
                format!("The blueprint for {name} v{version}. {effect}")
            }
            Self::Ram(size) => format!("{size} GB of RAM, not {} GB.", state.rig.ram),
            Self::Cpu(cycles) => format!("{cycles} cycles, not {}.", state.rig.cpu),
        }
    }
}

impl World {
    pub(super) fn is_shop(&self, host: HostId) -> bool {
        self.shops.iter().any(|shop| shop.host == host)
    }

    pub(super) fn ware_names(&self, player: PlayerId) -> Vec<&str> {
        let (Ok(shop), Some(state)) = (self.shop_here(player), self.players.get(&player)) else {
            return Vec::new();
        };
        shop.wares(state)
            .into_iter()
            .map(|(ware, _)| ware.name(shop))
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
                    .span(Style::Command, ware.name(shop))
                    .text(rest),
            );
        }
        let fence = if shop.fence.is_some() {
            let more = DARKNET_PAY - FULL;
            format!("The fence buys stolen data and pays {more}% more than in the city. Type ")
        } else {
            "The fence buys stolen data. Type ".to_owned()
        };
        shown.push(
            Message::game(fence)
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
        let (here, now) = (self.here(player), self.now);
        let fence = self
            .shops
            .iter_mut()
            .find(|shop| shop.host == here)
            .and_then(|shop| shop.fence.as_mut());
        let (market, pay) = match fence {
            Some(fence) => (fence, DARKNET_PAY),
            None => (&mut self.market, FULL),
        };
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        let names: Vec<String> = match (args, input) {
            ([], None) => return Err(wanted(&state.rig.files, market, pay, now)),
            ([], Some(items)) => items
                .into_iter()
                .filter_map(|item| match item {
                    Item::File(file) => Some(file.name),
                    Item::Route(_) | Item::Ice { .. } | Item::Rig { .. } | Item::Line(_) => None,
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
            let percent = market.percent(&name, now);
            let price = market.price(&name, value * pay / FULL, now);
            market.sell(&name, now);
            self.state_dirty = true;
            state.rig.files.remove(index);
            state.credits += price;
            state.sold.push(name.clone());
            let paid = Message::game(format!("The fence pays you {price} cr for "))
                .span(Style::File, &name);
            shown.push(if percent < FULL {
                paid.text(format!(", {percent}% of its full price."))
            } else {
                paid.text(".")
            });
        }
        Ok(Step::show(shown))
    }

    pub(super) fn buy(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let shop = self.shop_here(player)?;
        let Some(state) = self.players.get(&player) else {
            return Ok(Step::default());
        };
        let wares = shop.wares(state);
        let names: Vec<&str> = wares.iter().map(|(ware, _)| ware.name(shop)).collect();
        let [word] = args else {
            return Err(usage("buy <item>", Thing::Ware, &names));
        };
        let word = word.to_lowercase();
        if let Some(refusal) = shop.refusal(state, &word, &names) {
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
        let bought = format!("You buy {name} for {price} cr.");
        let home = self.home.map(|home| self.hosts[home.0].id.clone());
        let follow_up = |action: &str| match &home {
            Some(home) => format!("{bought} {action} it at {home}."),
            None => bought.clone(),
        };
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        state.credits -= price;
        let message = match ware {
            Ware::Program(verb) => {
                state.know.push(verb);
                follow_up("Load")
            }
            Ware::Blueprint(verb, version) => {
                state.blueprints.push((verb, version));
                follow_up("Compile")
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

fn wanted(files: &[File], market: &Market, pay: u32, now: u64) -> Message {
    let mut message = Message::error("Usage: sell <file>. The fence wants: ");
    let valued: Vec<&File> = files.iter().filter(|file| file.value > 0).collect();
    if valued.is_empty() {
        return message.text("nothing on your rig");
    }
    for (index, file) in valued.iter().enumerate() {
        if index > 0 {
            message = message.text(", ");
        }
        let price = market.price(&file.name, file.value * pay / FULL, now);
        message = message
            .span(Style::File, &file.name)
            .text(format!(" ({price} cr)"));
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
                "exploit: 800 cr. Raise your access on this host by one level. It makes much noise. A corp patches software that gets many exploits.",
                "crack-v2: 600 cr. The blueprint for crack v2. It does 20 damage.",
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
            state.know.push(crate::commands::Verb::Exploit);
            state.blueprints.push((crate::commands::Verb::Crack, 2));
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
    fn a_bought_blueprint_joins_the_account() {
        let (mut world, player) = rich("ghost", 700);
        assert_eq!(
            plain(&world.command(player, "buy crack-v2")),
            ["You buy crack-v2 for 600 cr. Compile it at back-room."]
        );
        assert_eq!(
            plain(&world.command(player, "buy crack-v2")),
            ["You already have the blueprint for crack v2."]
        );
        assert_eq!(
            plain(&world.command(player, "shop | grep v2")),
            ["No match for v2."]
        );
        for line in ["back", "go back-room"] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "compile"))[0],
            "Blueprints: crack v2"
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
            ["The fence pays you 36 cr for payroll.db, 90% of its full price."]
        );
    }

    fn seller(world: &mut World, account: &str, handle: &str) -> PlayerId {
        let (player, _) = world.connect(account);
        for line in [handle, "ghost", "go market"] {
            world.command(player, line);
        }
        carry(world, player, "payroll.db", 40);
        player
    }

    #[test]
    fn each_sale_of_a_file_name_lowers_its_price_for_a_time() {
        let mut world = world();
        world.set_time(10_000);
        let (nyx, zero) = (
            seller(&mut world, "a", "nyx"),
            seller(&mut world, "b", "zer0"),
        );
        assert_eq!(
            plain(&world.command(nyx, "sell payroll.db")),
            ["The fence pays you 40 cr for payroll.db."]
        );
        assert_eq!(
            plain(&world.command(zero, "sell")),
            ["Usage: sell <file>. The fence wants: payroll.db (36 cr)"]
        );
        assert_eq!(
            plain(&world.command(zero, "sell payroll.db")),
            ["The fence pays you 36 cr for payroll.db, 90% of its full price."]
        );
        world.set_time(10_000 + 2 * 3_600);
        let moth = seller(&mut world, "c", "moth");
        assert_eq!(
            plain(&world.command(moth, "sell")),
            ["Usage: sell <file>. The fence wants: payroll.db (36 cr)"]
        );
    }

    #[test]
    fn data_prices_come_back_from_the_save() {
        let mut world = world();
        world.set_time(10_000);
        let nyx = seller(&mut world, "a", "nyx");
        world.command(nyx, "sell payroll.db");
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "world.ini")
            .map_or_default(|(_, text)| text.clone());
        assert!(text.contains("\nprice = payroll.db 90 10000\n"), "{text}");
        let mut fresh = crate::testing::world();
        fresh.set_time(10_000);
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        let zero = seller(&mut fresh, "b", "zer0");
        assert_eq!(
            plain(&fresh.command(zero, "sell")),
            ["Usage: sell <file>. The fence wants: payroll.db (36 cr)"]
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

    fn in_den(world: &mut World, name: &str) -> PlayerId {
        let player = crate::testing::inside(world, name);
        world.command(player, "go den");
        carry(world, player, "payroll.db", 40);
        player
    }

    #[test]
    fn a_darknet_fence_pays_half_again_and_has_its_own_prices() {
        let mut world = crate::testing::dark();
        world.set_time(10_000);
        let nyx = in_den(&mut world, "nyx");
        assert_eq!(
            plain(&world.command(nyx, "shop"))[1..3],
            [
                "crack-v3: 2500 cr. The blueprint for crack v3. It does 25 damage.",
                "The fence buys stolen data and pays 50% more than in the city. Type sell <file>.",
            ]
        );
        assert_eq!(
            plain(&world.command(nyx, "sell payroll.db")),
            ["The fence pays you 60 cr for payroll.db."]
        );
        let zer0 = in_den(&mut world, "zer0");
        assert_eq!(
            plain(&world.command(zer0, "sell")),
            ["Usage: sell <file>. The fence wants: payroll.db (54 cr)"]
        );
        let (ghost, _) = world.join("ghost");
        carry(&mut world, ghost, "payroll.db", 40);
        assert_eq!(
            plain(&world.command(ghost, "sell payroll.db")),
            ["The fence pays you 40 cr for payroll.db."]
        );
    }

    #[test]
    fn darknet_fence_prices_come_back_from_the_save() {
        let mut world = crate::testing::dark();
        world.set_time(10_000);
        let nyx = in_den(&mut world, "nyx");
        world.command(nyx, "sell payroll.db");
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "world.ini")
            .map_or_default(|(_, text)| text.clone());
        assert!(
            text.ends_with("[world d]\nseed = 0\n\n[fence den]\nprice = payroll.db 90 10000\n"),
            "{text}"
        );
        let mut fresh = crate::testing::dark();
        fresh.set_time(10_000);
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        let zer0 = in_den(&mut fresh, "zer0");
        assert_eq!(
            plain(&fresh.command(zer0, "sell")),
            ["Usage: sell <file>. The fence wants: payroll.db (54 cr)"]
        );
    }

    #[test]
    fn a_shop_can_sell_a_blueprint_for_version_3() {
        let mut world = crate::testing::dark();
        let nyx = in_den(&mut world, "nyx");
        if let Some(state) = world.players.get_mut(&nyx) {
            state.credits = 2_500;
        }
        assert_eq!(
            plain(&world.command(nyx, "buy crack-v3")),
            ["You buy crack-v3 for 2500 cr. Compile it at home."]
        );
        assert_eq!(
            plain(&world.command(nyx, "buy crack-v3")),
            ["You already have the blueprint for crack v3."]
        );
        let blueprints = world
            .players
            .get(&nyx)
            .map(|state| state.blueprints.clone());
        assert_eq!(blueprints, Some(vec![(crate::commands::Verb::Crack, 3)]));
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
        let wares = "patch, firewall, exploit, crack-v2, ram, cpu";
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
