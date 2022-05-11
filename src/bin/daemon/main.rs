use std::{pin::Pin, future::Future};

use clap::Parser;

use mccloud::{
    config::Config,
    network::{
        peer::Peer,
        handler::daemon::{DaemonHandler, UserDataHandler}
    }, 
};

#[derive(Parser)]
struct Args {
    #[clap(long, short)]
    config: String,
}

#[derive(Clone)]
pub struct UserData {}

impl UserDataHandler for UserData {
    type UserData = ();

    fn new() -> Self {
        Self {}
    }

    fn handle<'a>(&'a self, _data: Self::UserData) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
            async fn run() {}

            Box::pin(run())
        }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let env = env_logger::Env::default().default_filter_or("debug");
    env_logger::init_from_env(env);

    let config = Config::load(&args.config).await.unwrap();
    let peer = Peer::<DaemonHandler<UserData>>::new(config);

    if let Err(e) = peer.listen().await {
        log::error!("{}", e);
    }
}
