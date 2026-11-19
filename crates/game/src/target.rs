use crate::content::{CorpDef, File, IceDef, TemplateDef};
use crate::heat::Security;
use crate::ice::IceKind;
use crate::random::Random;

const LAN_VALUE: u32 = 40;
const CORE_VALUE: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    Edge,
    Lan,
    Core,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) name: String,
    pub(crate) hosts: Vec<TargetHost>,
    pub(crate) goal: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetHost {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) layer: Layer,
    pub(crate) security: Security,
    pub(crate) routes: Vec<usize>,
    pub(crate) files: Vec<File>,
    pub(crate) ice: Vec<IceDef>,
}

pub(crate) fn security(budget: Security, layer: Layer) -> Security {
    use Security::{High, Low, Medium, Top};
    match (budget, layer) {
        (Low, Layer::Edge | Layer::Lan) | (Medium, Layer::Edge) => Low,
        (Low, Layer::Core) | (Medium, Layer::Lan) | (High, Layer::Edge) => Medium,
        (Medium | High, Layer::Core) | (High, Layer::Lan) | (Top, Layer::Edge | Layer::Lan) => High,
        (Top, Layer::Core) => Top,
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
    };
    for (layer, least) in [(Layer::Edge, 1), (Layer::Lan, 2), (Layer::Core, 2)] {
        let count = least + random.below(2);
        target.place(corp, templates, layer, count, &mut random);
    }
    target.link(&mut random);
    target.deploy(&mut random);
    target.fill(corp, &mut random);
    target
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
        let mut pool: Vec<&TemplateDef> =
            templates.iter().filter(|def| def.layer == layer).collect();
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
            self.hosts.push(TargetHost {
                id,
                name: format!("{} {}", corp.name, template.name),
                description: template.description.clone(),
                layer,
                security: security(corp.budget, layer),
                routes: Vec::new(),
                files: template.files.clone(),
                ice: Vec::new(),
            });
        }
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
        let ice = |kind: IceKind, security: Security, guards: Option<String>| IceDef {
            name: kind.title().to_lowercase(),
            kind,
            version: version(security),
            guards,
        };
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
                Layer::Edge | Layer::Core => CORE_VALUE * factor,
            };
            self.hosts[host].files.push(file);
        }
    }
}

impl Layer {
    pub(crate) const ALL: [Self; 3] = [Self::Edge, Self::Lan, Self::Core];

    pub(crate) fn find(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layer| layer.word() == word)
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Edge => "edge",
            Self::Lan => "lan",
            Self::Core => "core",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Layer, Target, generate, security};
    use crate::Content;
    use crate::content::File;
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
        assert_eq!(Layer::find("vault"), None);
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
