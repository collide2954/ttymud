use super::reader::Reader;
use super::{Content, ContentError, HostDef, Zone};

impl Reader {
    pub(super) fn finish(mut self) -> Result<Content, Vec<ContentError>> {
        self.place_files();
        self.place_ice();
        self.check_networks();
        self.check_routes();
        self.check_city();
        self.check_contracts();
        self.check_templates();
        self.check_fixers();
        self.errors.sort_by_key(|error| error.line);
        if !self.errors.is_empty() {
            return Err(self.errors);
        }
        Ok(Content {
            id: self.city_id,
            city: self.city_name.unwrap_or_default(),
            start: self.start.unwrap_or_default().0,
            home: self.home.map(|(home, _)| home),
            burns: self.burns.map(|(host, _)| host),
            news: self.news.map(|(host, _)| host),
            auction: self.auction.map(|(host, _)| host),
            contract: self.contract.map(|(id, _)| id),
            networks: self
                .networks
                .into_iter()
                .map(|(network, _)| network)
                .collect(),
            hosts: self.hosts.into_iter().map(|(host, _)| host).collect(),
            contracts: self.contracts.into_iter().map(|(def, ..)| def).collect(),
            shops: self.shops.into_iter().map(|(shop, _)| shop).collect(),
            corps: self.corps.into_iter().map(|(corp, _)| corp).collect(),
            templates: self
                .templates
                .into_iter()
                .map(|(template, _)| template)
                .collect(),
            fixers: self.fixers.into_iter().map(|(fixer, _)| fixer).collect(),
            channels: self.channels,
        })
    }

    fn check_contracts(&mut self) {
        let mut problems = Vec::new();
        for (def, [fixer_line, network_line, goal_line]) in &self.contracts {
            let (id, network, goal) = (&def.id, &def.network, &def.goal);
            if !self.fixers.iter().any(|(fixer, _)| fixer.id == def.fixer) {
                let message = format!("contract {id} has an unknown fixer: {}", def.fixer);
                problems.push((*fixer_line, message));
            }
            if !self.networks.iter().any(|(known, _)| &known.id == network) {
                let message = format!("contract {id} is for an unknown network: {network}");
                problems.push((*network_line, message));
                continue;
            }
            let found = self.hosts.iter().any(|(host, _)| {
                host.network.as_ref() == Some(network)
                    && host.files.iter().any(|file| &file.name == goal)
            });
            if !found {
                let message = format!("goal {goal} is not a file in network {network}");
                problems.push((*goal_line, message));
            }
        }
        if let Some((id, line)) = &self.contract
            && !self.contracts.iter().any(|(def, ..)| &def.id == id)
        {
            problems.push((*line, format!("contract is an unknown contract: {id}")));
        }
        for (line, message) in problems {
            self.error(line, message);
        }
    }

    fn position(&self, host: &str) -> Option<usize> {
        self.hosts.iter().position(|(def, _)| def.id == host)
    }

    fn linked(&self, a: &str, b: &str) -> bool {
        let routes = |from: &str, to: &str| {
            self.hosts
                .iter()
                .any(|(def, _)| def.id == from && def.routes.iter().any(|route| route == to))
        };
        routes(a, b) || routes(b, a)
    }

    fn place_files(&mut self) {
        for (host, file, line) in std::mem::take(&mut self.files) {
            let name = &file.name;
            let template = self.templates.iter().position(|(def, _)| def.id == host);
            let files = match (self.position(&host), template) {
                (Some(index), _) => &mut self.hosts[index].0.files,
                (None, Some(index)) => &mut self.templates[index].0.files,
                (None, None) => {
                    self.error(line, format!("file is on an unknown host: {host}"));
                    continue;
                }
            };
            if files.iter().any(|known| known.name == file.name) {
                self.error(
                    line,
                    format!("file {host}/{name} is defined more than once"),
                );
            } else {
                files.push(file);
            }
        }
    }

    fn place_ice(&mut self) {
        for (host, ice, line) in std::mem::take(&mut self.ice) {
            let name = &ice.name;
            let Some(index) = self.position(&host) else {
                self.error(line, format!("ice is on an unknown host: {host}"));
                continue;
            };
            let def = &self.hosts[index].0;
            if def.network.is_none() {
                let message = format!("ice must be on a host in a network: {host}/{name}");
                self.error(line, message);
            } else if def.ice.iter().any(|known| known.name == ice.name) {
                self.error(line, format!("ice {host}/{name} is defined more than once"));
            } else if let Some(target) = ice.guards.as_deref()
                && !self.linked(&host, target)
            {
                self.error(line, format!("{host}/{name} guards no route to {target}"));
            } else {
                self.hosts[index].0.ice.push(ice);
            }
        }
    }

    fn check_networks(&mut self) {
        for (host, network, line) in std::mem::take(&mut self.members) {
            if !self.networks.iter().any(|(def, _)| def.id == network) {
                let message = format!("host {host} is in an unknown network: {network}");
                self.error(line, message);
            }
        }
        let empty: Vec<(String, usize)> = self
            .networks
            .iter()
            .filter(|(def, _)| self.members_of(&def.id) == 0)
            .map(|(def, line)| (def.id.clone(), *line))
            .collect();
        for (id, line) in empty {
            self.error(line, format!("network {id} has no hosts"));
        }
        self.check_bases();
    }

    fn check_bases(&mut self) {
        let bases = std::mem::take(&mut self.bases);
        let mut problems = Vec::new();
        for (network, host, line) in &bases {
            if self.zone_of(network) != Some(Zone::Arena) {
                problems.push((*line, format!("only an arena has bases: {network}")));
            } else if self.network_of(host) != Some(network.as_str()) {
                problems.push((
                    *line,
                    format!("base {host} is not a host in arena {network}"),
                ));
            }
        }
        for (def, line) in self
            .networks
            .iter()
            .filter(|(def, _)| def.zone == Zone::Arena)
        {
            let named: Vec<&str> = bases
                .iter()
                .filter(|(network, ..)| *network == def.id)
                .map(|(_, host, _)| host.as_str())
                .collect();
            if !matches!(named.as_slice(), [one, other] if one != other) {
                problems.push((*line, format!("arena {} needs two bases", def.id)));
            }
        }
        for (line, message) in problems {
            self.error(line, message);
        }
    }

    fn check_routes(&mut self) {
        for (from, to, line) in std::mem::take(&mut self.routes) {
            let (a, b) = (self.network_of(&from), self.network_of(&to));
            let crossing = matches!((a, b), (Some(a), Some(b)) if a != b);
            let arena = |network: Option<&str>| {
                network.is_some_and(|network| self.zone_of(network) == Some(Zone::Arena))
            };
            if from == to {
                self.error(line, format!("host {from} has a route to itself"));
            } else if !self.has_host(&to) {
                self.error(line, format!("route to an unknown host: {to}"));
            } else if crossing {
                self.error(line, format!("route between two networks: {from} and {to}"));
            } else if arena(a) != arena(b) {
                self.error(line, format!("route into an arena: {from} and {to}"));
            }
        }
    }

    fn check_city(&mut self) {
        if self.city.is_none() {
            self.error(0, "no city section");
        }
        if let Some((start, line)) = self.start.clone() {
            if self.has_host(&start) {
                let lost: Vec<(String, usize)> = unreachable(&self.hosts, &start)
                    .into_iter()
                    .filter(|(id, _)| {
                        let network = self.network_of(id);
                        network.and_then(|network| self.zone_of(network)) != Some(Zone::Arena)
                    })
                    .collect();
                for (id, line) in lost {
                    let message = format!("host {id} cannot be reached from the start host");
                    self.error(line, message);
                }
            } else {
                self.error(line, format!("start is an unknown host: {start}"));
            }
        }
        let shops = self
            .shops
            .iter()
            .map(|(shop, line)| ("shop", Some((shop.host.clone(), *line))));
        let places: Vec<(&str, Option<(String, usize)>)> = [
            ("home", self.home.clone()),
            ("burns", self.burns.clone()),
            ("news", self.news.clone()),
            ("auction", self.auction.clone()),
        ]
        .into_iter()
        .chain(shops)
        .collect();
        for (key, value) in places {
            let Some((host, line)) = value else {
                continue;
            };
            let zone = self.network_of(&host).map(|network| self.zone_of(network));
            match zone {
                _ if !self.has_host(&host) => {
                    self.error(line, format!("{key} is an unknown host: {host}"));
                }
                Some(Some(Zone::Darknet)) if key == "shop" => {}
                Some(Some(Zone::Arena)) if key == "shop" => {
                    self.error(line, format!("shop is in an arena: {host}"));
                }
                Some(_) if key == "shop" => {
                    self.error(line, format!("shop is in a target network: {host}"));
                }
                Some(_) => self.error(line, format!("{key} is in a network: {host}")),
                None => {}
            }
        }
    }
}

fn unreachable(hosts: &[(HostDef, usize)], start: &str) -> Vec<(String, usize)> {
    let mut seen = vec![false; hosts.len()];
    let mut queue: Vec<usize> = hosts
        .iter()
        .position(|(host, _)| host.id == start)
        .into_iter()
        .collect();
    for &index in &queue {
        seen[index] = true;
    }
    while let Some(current) = queue.pop() {
        let here = &hosts[current].0;
        for (index, (host, _)) in hosts.iter().enumerate() {
            let linked = here.routes.contains(&host.id) || host.routes.contains(&here.id);
            if linked && !seen[index] {
                seen[index] = true;
                queue.push(index);
            }
        }
    }
    hosts
        .iter()
        .zip(seen)
        .filter(|(_, reached)| !reached)
        .map(|((host, line), _)| (host.id.clone(), *line))
        .collect()
}
