
use clap::Parser;
use cluster_rs::{
    key::Key,
    network::peer::Peer,
    config::{Config, ClientConfig}
};

mod handler;
use handler::CliHandler;

/// cli connection to the cluster
#[derive(Parser)]
#[clap(author, version, about)]
struct Args {
    /// host name or ip
    #[clap(long, short)]
    host: String,
    /// the rpc port
    #[clap(long, short)]
    port: u16,
    /// the wallet of the user
    #[clap(long, short)]
    wallet: String,
}


#[tokio::main]
async fn main() {
    let args = Args::parse();

    let env = env_logger::Env::default().default_filter_or("debug");
    env_logger::init_from_env(env);

    let _key = Key::load(&args.wallet).unwrap();
    let config = Config {
        thin: true,
        host: "127.0.0.1".to_string(),
        port: 9999,
        clients: vec![ClientConfig {
            host: args.host.clone(),
            port: args.port,
        }],
    };
    let peer = Peer::<CliHandler>::new(config);

    if let Err(e) = peer.listen().await {
        log::error!("{}", e);
    }

    log::info!("-- done --");
}