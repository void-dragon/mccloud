use std::{pin::Pin, future::Future};

use mccloud::{
    network::{
        client::ClientPtr,
        peer::Peer,
        handler::Handler,
        message::Message,
    },
    blockchain::Data,
    config::Config
};


macro_rules! check {
    ($ex:expr) => {
        if let Err(e) = $ex {
            log::error!("{}", e);
        }
    };
}


#[derive(Clone)]
pub struct TestHandler {
}

impl TestHandler {
}

impl Handler for TestHandler {
    fn new(_config: &Config) -> Self {
        Self { }
    }

    fn init<'a>(&'a self, peer: Peer<Self>, client: ClientPtr) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
        async fn run(_self: &TestHandler, peer: Peer<TestHandler>, client: ClientPtr) {
            log::debug!("init");
            let data = b"shrouble".to_vec();
            let data = Data::build(&peer.key, data);
            let msg = Message::Share { data };
            let data = msg.to_bytes().unwrap();
            log::debug!("send data share");
            check!(client.write_aes(&data).await);
        }

        Box::pin(run(self, peer, client)) 
    }

    fn shutdown<'a>(&'a self, peer: Peer<Self>) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
        async fn run(_self: &TestHandler, _peer: Peer<TestHandler>) {
        }

        Box::pin(run(self, peer)) 
    }
    
    fn handle<'a>(&'a self, peer: Peer<Self>, client: ClientPtr, msg: Message) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
        async fn run(_self: &TestHandler, _peer: Peer<TestHandler>, _client: ClientPtr, msg: Message) {
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