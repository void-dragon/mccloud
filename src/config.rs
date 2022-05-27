use std::path::Path;

use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Clone)]
pub struct ClientConfig {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub reconnect: bool,
}

///
/// The configuration of a single [Peer](`crate::network::peer::Peer`).
/// 
/// **TOML Example:**
/// ```toml
/// host = "127.0.0.1"
/// port = 39093
/// folder = "data/"
/// thin = false
/// clients = []
/// ```
/// 
#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    /// The ip on which to listen. Defaults to `127.0.0.1`.
    pub host: String,
    /// The port to listen on. Defaults to `39093`.
    pub port: u16,
    /// Indicates if the node is full participant or only retrives data.
    pub thin: bool,
    /// Folder where to store data. Defaults to `data/`.
    pub folder: String,
    /// The other nodes to connect to.
    pub clients: Vec<ClientConfig>
}

impl Config {
    ///
    /// Load [Config] from filename.
    ///
    pub async fn load(filename: &str) -> Result<Config, anyhow::Error> {
        let data = tokio::fs::read(filename).await?;
        Ok(toml::from_slice(&data)?)
    }

    pub async fn save(&self, filename: &str) -> Result<(), anyhow::Error> {
        let path = Path::new(filename);
        let folder = path.parent().unwrap();
        
        tokio::fs::create_dir_all(folder).await?;

        let data = toml::to_string_pretty(self)?;
        tokio::fs::write(path, &data).await?;

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: "127.0.0.1".to_owned(),
            port: 39093,
            thin: false,
            folder: "data/".to_owned(),
            clients: Vec::new(),
        }
    }
}