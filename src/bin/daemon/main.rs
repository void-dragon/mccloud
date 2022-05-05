use clap::Parser;

use cluster_rs::{
    config::Config,
    network::peer::Peer, 
};
mod handler;
use handler::DaemonHandler;

#[derive(Parser)]
struct Args {
    #[clap(long, short)]
    config: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let env = env_logger::Env::default().default_filter_or("debug");
    env_logger::init_from_env(env);

    let config = Config::load(&args.config).await.unwrap();
    let peer = Peer::<DaemonHandler>::new(config);

    if let Err(e) = peer.listen().await {
        log::error!("{}", e);
    }

    log::info!("-- done --");
}
