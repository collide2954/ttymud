#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub host: String,
    pub run: Option<RunStatus>,
    pub integrity: u32,
    pub heat: u32,
    pub heat_max: u32,
    pub ram_used: u32,
    pub ram_total: u32,
    pub cycles: u32,
    pub cycles_max: u32,
    pub credits: u32,
    pub headline: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunStatus {
    pub network: String,
    pub access: String,
    pub trace: u32,
    pub alert: String,
}
