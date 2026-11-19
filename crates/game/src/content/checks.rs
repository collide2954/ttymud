use super::reader::Reader;
use super::{Content, ContentError, HostDef};

impl Reader {
    pub(super) fn finish(mut self) -> Result<Content, Vec<ContentError>> {
        for (host, file, line) in std::mem::take(&mut self.files) {
            let position = self.hosts.iter().position(|(def, _)| def.id == host);
            match position {
                None => self.error(line, format!("file is on an unknown host: {host}")),
                Some(index)
                    if self.hosts[index]
                        .0
                        .files
                        .iter()
                        .any(|f| f.name == file.name) =>
                {
                    let name = &file.name;
                    self.error(
                        line,
                        format!("file {host}/{name} is defined more than once"),
                    );
                }
                Some(index) => self.hosts[index].0.files.push(file),
            }
        }
        for (host, network, line) in std::mem::take(&mut self.members) {
            if !self.networks.iter().any(|(def, _)| def.id == network) {
                self.error(
                    line,
                    format!("host {host} is in an unknown network: {network}"),
                );
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
        for (from, to, line) in std::mem::take(&mut self.routes) {
            let crossing = matches!(
                (self.network_of(&from), self.network_of(&to)),
                (Some(a), Some(b)) if a != b
            );
            if from == to {
                self.error(line, format!("host {from} has a route to itself"));
            } else if !self.has_host(&to) {
                self.error(line, format!("route to an unknown host: {to}"));
            } else if crossing {
                self.error(line, format!("route between two networks: {from} and {to}"));
            }
        }
        if self.city.is_none() {
            self.error(0, "no city section");
        }
        if let Some((start, line)) = self.start.clone() {
            if self.has_host(&start) {
                for (id, line) in unreachable(&self.hosts, &start) {
                    self.error(
                        line,
                        format!("host {id} cannot be reached from the start host"),
                    );
                }
            } else {
                self.error(line, format!("start is an unknown host: {start}"));
            }
        }
        if let Some((home, line)) = self.home.clone() {
            if !self.has_host(&home) {
                self.error(line, format!("home is an unknown host: {home}"));
            } else if self.network_of(&home).is_some() {
                self.error(line, format!("home is in a network: {home}"));
            }
        }
        self.errors.sort_by_key(|error| error.line);
        if !self.errors.is_empty() {
            return Err(self.errors);
        }
        Ok(Content {
            city: self.city_name.unwrap_or_default(),
            start: self.start.unwrap_or_default().0,
            home: self.home.map(|(home, _)| home),
            networks: self
                .networks
                .into_iter()
                .map(|(network, _)| network)
                .collect(),
            hosts: self.hosts.into_iter().map(|(host, _)| host).collect(),
        })
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
