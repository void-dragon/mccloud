use std::error::Error;

use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Clone)]
pub struct ClientConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub clients: Vec<ClientConfig>
}

impl Config {
    pub fn load(filename: &str) -> Result<Config, Box<dyn Error>> {
        let data = std::fs::read(filename)?;
        Ok(toml::from_slice(&data)?)
    }
}