use crate::commands::{self, Verb};
use crate::content::{CorpDef, File, IceDef, TemplateDef};
use crate::heat::Security;
use crate::ice::IceKind;
use crate::names::person;
use crate::random::{Random, mix};

const HEIST_SALT: u64 = 0x7661;
const BLUEPRINT_SALT: u64 = 0x6270;
const LAN_VALUE: u32 = 40;
const CORE_VALUE: u32 = 100;
const WORDS: [&str; 12] = [
    "amber", "cobalt", "delta", "ember", "falcon", "harbor", "indigo", "juniper", "lantern",
    "marble", "orchid", "raven",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    Edge,
    Lan,
    Core,
    Vault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Vault {
    pub(crate) host: usize,
    pub(crate) laptop: usize,
    pub(crate) bridge: RoleBridge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RoleBridge {
    Employee { host: usize, name: String },
    Modem { host: usize },
}

impl Vault {
    pub(crate) fn moved(&self, base: usize) -> Self {
        let bridge = match &self.bridge {
            RoleBridge::Employee { host, name } => RoleBridge::Employee {
                host: base + host,
                name: name.clone(),
            },
            RoleBridge::Modem { host } => RoleBridge::Modem { host: base + host },
        };
        Self {
            host: base + self.host,
            laptop: base + self.laptop,
            bridge,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Level {
    Guest,
    User,
    Admin,
}

impl Level {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Guest => "guest",
            Self::User => "user",
            Self::Admin => "admin",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) name: String,
    pub(crate) hosts: Vec<TargetHost>,
    pub(crate) goal: String,
    pub(crate) keys: Vec<(String, Level)>,
    pub(crate) vault: Option<Vault>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetHost {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) layer: Layer,
    pub(crate) security: Security,
    pub(crate) software: Option<String>,
    pub(crate) routes: Vec<usize>,
    pub(crate) files: Vec<File>,
    pub(crate) ice: Vec<IceDef>,
}

pub(crate) fn security(budget: Security, layer: Layer) -> Security {
    use Security::{High, Low, Medium, Top};
    match (budget, layer) {
        (Low, Layer::Edge | Layer::Lan) | (Medium, Layer::Edge) => Low,
        (Low, Layer::Core) | (Medium, Layer::Lan) | (High, Layer::Edge) => Medium,
        (Medium | High, Layer::Core)
        | (Low | Medium, Layer::Vault)
        | (High, Layer::Lan)
        | (Top, Layer::Edge | Layer::Lan) => High,
        (Top, Layer::Core) | (High | Top, Layer::Vault) => Top,
    }
}

fn version(security: Security) -> u32 {
    match security {
        Security::Low => 1,
        Security::Medium => 2,
        Security::High => 3,
        Security::Top => 4,
    }
}

fn factor(budget: Security) -> u32 {
    match budget {
        Security::Low => 1,
        Security::Medium => 2,
        Security::High => 4,
        Security::Top => 8,
    }
}

pub(crate) fn generate(corp: &CorpDef, templates: &[TemplateDef], seed: u64) -> Target {
    let mut random = Random::new(seed);
    let mut target = Target {
        name: corp.name.clone(),
        hosts: Vec::new(),
        goal: String::new(),
        keys: Vec::new(),
        vault: None,
    };
    for (layer, least) in [(Layer::Edge, 1), (Layer::Lan, 2), (Layer::Core, 2)] {
        let count = least + random.below(2);
        target.place(corp, templates, layer, count, &mut random);
    }
    target.link(&mut random);
    target.deploy(&mut random);
    target.fill(corp, &mut random);
    if Random::new(mix(&[seed, BLUEPRINT_SALT])).below(3) == 0 {
        target.plant(seed, Layer::Core, 2);
    }
    target.lock();
    target.leak(&mut random);
    target
}

pub(crate) fn generate_heist(
    corp: &CorpDef,
    templates: &[TemplateDef],
    seed: u64,
) -> Option<Target> {
    let vaults: Vec<&TemplateDef> = templates
        .iter()
        .filter(|def| def.layer == Layer::Vault)
        .collect();
    let laptops: Vec<&TemplateDef> = templates.iter().filter(|def| def.laptop).collect();
    if vaults.is_empty() || laptops.is_empty() {
        return None;
    }
    let mut target = generate(corp, templates, seed);
    let mut random = Random::new(mix(&[seed, HEIST_SALT]));
    let (lan, deep) = (target.layer(Layer::Lan), target.layer(Layer::Core));
    if lan.is_empty() || deep.is_empty() {
        return None;
    }
    let pick = |random: &mut Random, from: &[usize]| from[random.below(from.len())];
    let laptop = laptops[random.below(laptops.len())];
    let laptop = target.add_host(corp, laptop, Layer::Lan, laptop.id.clone());
    let near = pick(&mut random, &lan);
    target.connect(laptop, near);
    let safe = vaults[random.below(vaults.len())];
    let safe = target.add_host(corp, safe, Layer::Vault, safe.id.clone());
    let security = target.hosts[safe].security;
    target.hosts[safe].ice.extend([
        ice_def(IceKind::Sentry, security, None),
        ice_def(IceKind::Scorch, security, None),
    ]);
    let goal = target.goal.clone();
    let moved: Vec<File> = target
        .hosts
        .iter_mut()
        .flat_map(|host| {
            let (goals, rest) = std::mem::take(&mut host.files)
                .into_iter()
                .partition(|file: &File| file.name == goal);
            host.files = rest;
            goals
        })
        .collect();
    target.hosts[safe].files.extend(moved);
    target.plant(seed, Layer::Vault, 3);
    target.lock();
    let bridge = if random.below(2) == 0 {
        let host = pick(&mut random, &lan);
        RoleBridge::Employee {
            host,
            name: person(&mut random),
        }
    } else {
        RoleBridge::Modem {
            host: pick(&mut random, &deep),
        }
    };
    target.vault = Some(Vault {
        host: safe,
        laptop,
        bridge,
    });
    Some(target)
}

fn ice_def(kind: IceKind, security: Security, guards: Option<String>) -> IceDef {
    IceDef {
        name: kind.title().to_lowercase(),
        kind,
        version: version(security),
        guards,
    }
}

fn password(random: &mut Random) -> String {
    let word = WORDS[random.below(WORDS.len())];
    format!("{word}-{}", 1000 + random.below(9000))
}

impl Target {
    fn layer(&self, layer: Layer) -> Vec<usize> {
        (0..self.hosts.len())
            .filter(|&index| self.hosts[index].layer == layer)
            .collect()
    }

    fn place(
        &mut self,
        corp: &CorpDef,
        templates: &[TemplateDef],
        layer: Layer,
        count: usize,
        random: &mut Random,
    ) {
        let mut pool: Vec<&TemplateDef> = templates
            .iter()
            .filter(|def| def.layer == layer && !def.laptop)
            .collect();
        if pool.is_empty() {
            return;
        }
        random.shuffle(&mut pool);
        let picked: Vec<&TemplateDef> = (0..count).map(|index| pool[index % pool.len()]).collect();
        for (index, template) in picked.iter().enumerate() {
            let same = |other: &&&TemplateDef| other.id == template.id;
            let id = if picked.iter().filter(same).count() > 1 {
                let number = picked[..=index].iter().filter(same).count();
                format!("{}-{number}", template.id)
            } else {
                template.id.clone()
            };
            self.add_host(corp, template, layer, id);
        }
    }

    fn add_host(
        &mut self,
        corp: &CorpDef,
        template: &TemplateDef,
        layer: Layer,
        id: String,
    ) -> usize {
        self.hosts.push(TargetHost {
            id,
            name: format!("{} {}", corp.name, template.name),
            description: template.description.clone(),
            layer,
            security: security(corp.budget, layer),
            software: template.software.clone(),
            routes: Vec::new(),
            files: template.files.clone(),
            ice: Vec::new(),
        });
        self.hosts.len() - 1
    }

    fn connect(&mut self, a: usize, b: usize) {
        if !self.hosts[a].routes.contains(&b) {
            self.hosts[a].routes.push(b);
            self.hosts[b].routes.push(a);
        }
    }

    fn link(&mut self, random: &mut Random) {
        let (edge, lan, core) = (
            self.layer(Layer::Edge),
            self.layer(Layer::Lan),
            self.layer(Layer::Core),
        );
        if lan.is_empty() {
            return;
        }
        for &host in &edge {
            self.connect(host, lan[random.below(lan.len())]);
        }
        for pair in lan.windows(2) {
            self.connect(pair[0], pair[1]);
        }
        let mut reached = lan;
        for host in core {
            self.connect(reached[random.below(reached.len())], host);
            reached.push(host);
        }
    }

    fn deploy(&mut self, random: &mut Random) {
        let ice = ice_def;
        for host in self.layer(Layer::Edge) {
            let security = self.hosts[host].security;
            self.hosts[host]
                .ice
                .push(ice(IceKind::Sentry, security, None));
        }
        let lan = self.layer(Layer::Lan);
        if !lan.is_empty() {
            let hound = lan[random.below(lan.len())];
            let security = self.hosts[hound].security;
            self.hosts[hound]
                .ice
                .push(ice(IceKind::Hound, security, None));
            if version(security) > 1
                && let Some(&watched) = lan.iter().find(|&&host| host != hound)
            {
                self.hosts[watched]
                    .ice
                    .push(ice(IceKind::Sentry, security, None));
            }
        }
        let core = self.layer(Layer::Core);
        if let Some(&front) = core.first()
            && let Some(&gate) = self.hosts[front]
                .routes
                .iter()
                .find(|&&host| self.hosts[host].layer == Layer::Lan)
        {
            let security = self.hosts[front].security;
            let guarded = Some(self.hosts[front].id.clone());
            self.hosts[gate]
                .ice
                .push(ice(IceKind::Warden, security, guarded));
        }
        for host in core {
            let security = self.hosts[host].security;
            if version(security) > 1 {
                self.hosts[host]
                    .ice
                    .push(ice(IceKind::Scorch, security, None));
            }
        }
    }

    fn fill(&mut self, corp: &CorpDef, random: &mut Random) {
        let core_hosts = self.layer(Layer::Core);
        let mut data = corp.data.clone();
        random.shuffle(&mut data);
        let mut data = data.into_iter();
        if let (Some(goal), false) = (data.next(), core_hosts.is_empty()) {
            self.goal.clone_from(&goal.name);
            let holder = core_hosts[random.below(core_hosts.len())];
            self.hosts[holder].files.push(goal);
        }
        let factor = factor(corp.budget);
        let hosts = self.layer(Layer::Lan).into_iter().chain(core_hosts);
        for (host, mut file) in hosts.zip(data) {
            file.value = match self.hosts[host].layer {
                Layer::Lan => LAN_VALUE * factor,
                Layer::Edge | Layer::Core | Layer::Vault => CORE_VALUE * factor,
            };
            self.hosts[host].files.push(file);
        }
    }

    fn plant(&mut self, seed: u64, layer: Layer, version: u32) {
        let hosts = self.layer(layer);
        let programs: Vec<Verb> = commands::versioned().collect();
        if hosts.is_empty() || programs.is_empty() {
            return;
        }
        let mut random = Random::new(mix(&[seed, BLUEPRINT_SALT, u64::from(version)]));
        let verb = programs[random.below(programs.len())];
        let host = hosts[random.below(hosts.len())];
        self.hosts[host].files.push(File::blueprint(verb, version));
    }

    fn lock(&mut self) {
        for host in &mut self.hosts {
            for file in &mut host.files {
                file.lock = match host.layer {
                    Layer::Core | Layer::Vault => Level::Admin,
                    Layer::Lan if file.value > 0 => Level::User,
                    Layer::Lan | Layer::Edge => Level::Guest,
                };
            }
        }
    }

    fn leak(&mut self, random: &mut Random) {
        let leaks = [
            (
                Layer::Edge,
                Level::Guest,
                Level::User,
                "helpdesk.txt",
                "The help desk gives this password to new staff for the LAN:",
            ),
            (
                Layer::Lan,
                Level::User,
                Level::Admin,
                "it-notes.txt",
                "Domain admin password. Do not share:",
            ),
        ];
        for (layer, lock, level, name, line) in leaks {
            let hosts = self.layer(layer);
            if hosts.is_empty() {
                continue;
            }
            let key = password(random);
            let mut file = File::new(name, vec![line.to_owned(), key.clone()]);
            file.lock = lock;
            file.key = Some(key.clone());
            self.hosts[hosts[random.below(hosts.len())]]
                .files
                .push(file);
            self.keys.push((key, level));
        }
    }
}

impl Layer {
    pub(crate) const BASE: [Self; 3] = [Self::Edge, Self::Lan, Self::Core];
    const ALL: [Self; 4] = [Self::Edge, Self::Lan, Self::Core, Self::Vault];

    pub(crate) fn find(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layer| layer.word() == word)
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Edge => "edge",
            Self::Lan => "lan",
            Self::Core => "core",
            Self::Vault => "vault",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Layer, Level, RoleBridge, Target, generate, generate_heist, security};
    use crate::Content;
    use crate::content::{File, TemplateDef};
    use crate::heat::Security;
    use crate::ice::IceKind;
    use crate::testing::CITY;

    fn target(seed: u64) -> Target {
        let content = Content::parse(CITY).unwrap();
        generate(&content.corps[0], &content.templates, seed)
    }

    fn layer(target: &Target, layer: Layer) -> Vec<usize> {
        (0..target.hosts.len())
            .filter(|&index| target.hosts[index].layer == layer)
            .collect()
    }

    #[test]
    fn layers_are_found_by_their_content_word() {
        assert_eq!(Layer::find("lan"), Some(Layer::Lan));
        assert_eq!(Layer::find("vault"), Some(Layer::Vault));
        assert_eq!(Layer::find("attic"), None);
    }

    #[test]
    fn the_same_seed_makes_the_same_target() {
        assert_eq!(target(7), target(7));
        let shapes: Vec<Vec<String>> = (0..20)
            .map(|seed| target(seed).hosts.into_iter().map(|host| host.id).collect())
            .collect();
        assert!(shapes.iter().any(|shape| shape != &shapes[0]));
    }

    #[test]
    fn a_target_has_three_layers_and_five_to_eight_hosts() {
        for seed in 0..50 {
            let target = target(seed);
            let counts =
                [Layer::Edge, Layer::Lan, Layer::Core].map(|kind| layer(&target, kind).len());
            assert!((1..=2).contains(&counts[0]), "{seed}: {counts:?}");
            assert!((2..=3).contains(&counts[1]), "{seed}: {counts:?}");
            assert!((2..=3).contains(&counts[2]), "{seed}: {counts:?}");
            let mut seen = vec![false; target.hosts.len()];
            let mut queue = layer(&target, Layer::Edge);
            while let Some(index) = queue.pop() {
                if !std::mem::replace(&mut seen[index], true) {
                    queue.extend(&target.hosts[index].routes);
                }
            }
            assert!(seen.iter().all(|&reached| reached), "{seed}");
        }
    }

    #[test]
    fn repeated_templates_get_numbers() {
        for seed in 0..50 {
            let ids: Vec<String> = target(seed).hosts.into_iter().map(|host| host.id).collect();
            let mut unique = ids.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), ids.len(), "{ids:?}");
            if ids.iter().any(|id| id.starts_with("ws-")) {
                assert!(ids.contains(&"ws-1".to_owned()), "{ids:?}");
            }
        }
    }

    #[test]
    fn a_heist_target_adds_a_vault_without_routes_a_laptop_and_a_role_bridge() {
        let content = Content::parse(CITY).unwrap();
        let corp = &content.corps[0];
        let (mut employees, mut modems) = (0, 0);
        for seed in 0..30 {
            let plain = generate(corp, &content.templates, seed);
            assert!(plain.vault.is_none());
            assert!(
                plain
                    .hosts
                    .iter()
                    .all(|host| host.layer != Layer::Vault && !host.id.starts_with("maint"))
            );
            let heist = generate_heist(corp, &content.templates, seed).unwrap();
            let vault = heist.vault.clone().unwrap();
            let safe = &heist.hosts[vault.host];
            assert_eq!(safe.layer, Layer::Vault);
            assert_eq!(safe.routes, Vec::<usize>::new());
            assert!(
                heist
                    .hosts
                    .iter()
                    .all(|host| !host.routes.contains(&vault.host))
            );
            let kinds: Vec<IceKind> = safe.ice.iter().map(|ice| ice.kind).collect();
            assert_eq!(kinds, [IceKind::Sentry, IceKind::Scorch]);
            assert!(safe.ice.iter().all(|ice| ice.version == 3));
            let holders = heist
                .hosts
                .iter()
                .filter(|host| host.files.iter().any(|file| file.name == heist.goal))
                .count();
            assert_eq!(holders, 1);
            assert!(safe.files.iter().any(|file| file.name == heist.goal));
            let laptop = &heist.hosts[vault.laptop];
            assert_eq!(
                (laptop.layer, laptop.routes.is_empty()),
                (Layer::Lan, false)
            );
            match &vault.bridge {
                RoleBridge::Employee { host, name } => {
                    employees += 1;
                    assert_eq!(heist.hosts[*host].layer, Layer::Lan);
                    assert_ne!(*host, vault.laptop);
                    assert!(name.contains('.'), "{name}");
                }
                RoleBridge::Modem { host } => {
                    modems += 1;
                    assert_eq!(heist.hosts[*host].layer, Layer::Core);
                }
            }
        }
        assert!(employees > 0 && modems > 0, "{employees} {modems}");
        let templates: Vec<TemplateDef> = content
            .templates
            .iter()
            .filter(|def| def.layer != Layer::Vault)
            .cloned()
            .collect();
        assert_eq!(generate_heist(corp, &templates, 1), None);
    }

    #[test]
    fn about_one_job_target_in_three_has_a_v2_blueprint_on_a_core_host() {
        let mut programs = std::collections::BTreeSet::new();
        let mut planted = 0;
        for seed in 0..300 {
            let target = target(seed);
            for host in &target.hosts {
                for file in &host.files {
                    let Some((verb, version)) = file.teaches() else {
                        continue;
                    };
                    planted += 1;
                    programs.insert(format!("{verb:?}"));
                    assert_eq!(
                        (host.layer, file.lock, version),
                        (Layer::Core, Level::Admin, 2),
                        "{seed}"
                    );
                }
            }
        }
        assert!((70..=130).contains(&planted), "{planted}");
        assert!(programs.len() > 3, "{programs:?}");
    }

    #[test]
    fn every_heist_vault_has_a_v3_blueprint() {
        let content = Content::parse(CITY).unwrap();
        for seed in 0..30 {
            let heist = generate_heist(&content.corps[0], &content.templates, seed).unwrap();
            let vault = heist.vault.as_ref().map_or(0, |vault| vault.host);
            let safe = &heist.hosts[vault];
            let taught: Vec<u32> = safe
                .files
                .iter()
                .filter_map(File::teaches)
                .map(|(_, version)| version)
                .collect();
            assert_eq!(taught, [3], "{seed}");
            assert!(safe.files.iter().all(|file| file.lock == Level::Admin));
        }
    }

    #[test]
    fn hosts_run_the_software_of_their_template() {
        let content = Content::parse(CITY).unwrap();
        for seed in 0..20 {
            for host in target(seed).hosts {
                let template = host.id.split('-').next().unwrap_or_default();
                let def = content.templates.iter().find(|def| def.id == template);
                assert_eq!(host.software, def.and_then(|def| def.software.clone()));
            }
        }
        let software = |id: &str| {
            let def = content.templates.iter().find(|def| def.id == id);
            def.and_then(|def| def.software.clone())
        };
        assert_eq!(software("share"), software("ws"));
        assert_eq!(software("backup"), None);
    }

    #[test]
    fn hosts_get_the_security_of_their_layer() {
        assert_eq!(security(Security::Low, Layer::Core), Security::Medium);
        assert_eq!(security(Security::Medium, Layer::Edge), Security::Low);
        assert_eq!(security(Security::Top, Layer::Lan), Security::High);
        let target = target(3);
        let host = &target.hosts[layer(&target, Layer::Core)[0]];
        assert_eq!(host.security, Security::Medium);
        assert!(host.name.starts_with("Acme Corp "), "{}", host.name);
    }

    #[test]
    fn the_goal_waits_on_a_core_host_and_other_data_has_a_value() {
        for seed in 0..20 {
            let target = target(seed);
            let holders: Vec<usize> = (0..target.hosts.len())
                .filter(|&index| {
                    target.hosts[index]
                        .files
                        .iter()
                        .any(|file| file.name == target.goal)
                })
                .collect();
            assert_eq!(holders.len(), 1, "{seed}");
            assert_eq!(target.hosts[holders[0]].layer, Layer::Core);
            for host in &target.hosts {
                let data = |file: &&File| {
                    std::path::Path::new(&file.name)
                        .extension()
                        .is_some_and(|ext| ext == "db")
                };
                for file in host.files.iter().filter(data) {
                    let value = match (file.name == target.goal, host.layer) {
                        (true, _) => 0,
                        (false, Layer::Lan) => 40,
                        (false, _) => 100,
                    };
                    assert_eq!(file.value, value, "{seed}: {}", file.name);
                }
            }
        }
    }

    #[test]
    fn goals_spread_over_the_data_of_the_corp() {
        let mut counts = std::collections::BTreeMap::new();
        for seed in 0..400 {
            *counts.entry(target(seed).goal).or_insert(0) += 1;
        }
        assert_eq!(counts.len(), 4, "{counts:?}");
        assert!(counts.values().all(|&count| count > 60), "{counts:?}");
    }

    #[test]
    fn data_needs_access_and_credentials_give_it() {
        for seed in 0..20 {
            let target = target(seed);
            for host in &target.hosts {
                for file in &host.files {
                    let lock = match host.layer {
                        Layer::Core | Layer::Vault => Level::Admin,
                        Layer::Lan if file.key.is_some() || file.value > 0 => Level::User,
                        Layer::Lan | Layer::Edge => Level::Guest,
                    };
                    assert_eq!(file.lock, lock, "{seed}: {}", file.name);
                }
            }
            let keys: Vec<(Layer, Level)> = target
                .hosts
                .iter()
                .flat_map(|host| host.files.iter().map(move |file| (host.layer, file)))
                .filter_map(|(layer, file)| {
                    let key = file.key.as_ref()?;
                    let (_, level) = target.keys.iter().find(|(known, _)| known == key)?;
                    Some((layer, *level))
                })
                .collect();
            assert_eq!(
                keys,
                [(Layer::Edge, Level::User), (Layer::Lan, Level::Admin)],
                "{seed}"
            );
        }
    }

    #[test]
    fn ice_guards_each_layer() {
        for seed in 0..20 {
            let target = target(seed);
            let kinds = |index: usize| -> Vec<IceKind> {
                target.hosts[index].ice.iter().map(|ice| ice.kind).collect()
            };
            for index in layer(&target, Layer::Edge) {
                assert_eq!(kinds(index), [IceKind::Sentry], "{seed}");
            }
            let lan: Vec<IceKind> = layer(&target, Layer::Lan)
                .into_iter()
                .flat_map(kinds)
                .collect();
            assert_eq!(
                lan.iter().filter(|&&kind| kind == IceKind::Hound).count(),
                1
            );
            let warden = target
                .hosts
                .iter()
                .flat_map(|host| &host.ice)
                .find(|ice| ice.kind == IceKind::Warden)
                .and_then(|ice| ice.guards.clone());
            let core = layer(&target, Layer::Core);
            assert!(
                core.iter()
                    .any(|&index| Some(&target.hosts[index].id) == warden.as_ref())
            );
            for index in core {
                assert!(kinds(index).contains(&IceKind::Scorch), "{seed}");
                assert!(target.hosts[index].ice.iter().all(|ice| ice.version == 2));
            }
        }
    }
}
