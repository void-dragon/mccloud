use clap::Parser;
use cluster_rs::{config::Config, peer::Peer};

#[derive(Parser)]
struct Args {
    #[clap(long, short)]
    config: String,
}

fn main() {
    let args = Args::parse();

    let env = env_logger::Env::default().default_filter_or("debug");
    env_logger::init_from_env(env);

    let config = Config::load(&args.config).unwrap();
    let peer = Peer::new(config);

    peer.listen();
}
