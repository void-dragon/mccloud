
use std::{pin::Pin, future::Future};

use cluster_rs::{
    network::{
        peer::{ClientPtr, Handler, Peer},
    },
    messages::Messages
};


macro_rules! check {
    ($ex:expr) => {
        if let Err(e) = $ex {
            log::error!("{}", e);
        }
    };
}

#[derive(Clone)]
pub struct CliHandler {
}

impl CliHandler {
}

impl Handler for CliHandler {
    type Msg = Messages;

    fn new() -> Self {
        Self {
        }
    }
    
    fn handle<'a>(&'a self, peer: Peer<Self>, client: ClientPtr, msg: Self::Msg) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
        async fn run(_self: &CliHandler, peer: Peer<CliHandler>, client: ClientPtr, msg: Messages) {
            
            match msg {
                _ => {}
                // Messages::Play { game } => {
                // }
                // Messages::Share { data } => {
                // }
                // Messages::ShareBlock { block } => {
                // }
            }
        }

        Box::pin(run(self, peer, client, msg)) 
    }
}