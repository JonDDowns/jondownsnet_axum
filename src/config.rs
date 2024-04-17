use serde::{Deserialize, Serialize};
use serde_yaml;

pub fn read_cfg(filenm: &str) -> Config {
    let f = std::fs::File::open(filenm).expect("Config file could not be found.");
    let d: Config = serde_yaml::from_reader(f).expect("Something happened");
    d
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub username: String,
    pub password: String,
    pub cnxn_str: String,
    pub pem_dir: String,
    pub port_http: u16,
    pub port_https: u16,
    pub max_connections: u32,
}
