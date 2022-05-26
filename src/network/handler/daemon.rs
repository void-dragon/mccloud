use std::{sync::Arc, pin::Pin, future::Future};

use tokio::sync::Mutex;

use crate::{
    highlander::{Highlander, Game, GameResult},
    blockchain::{Blockchain, Data, Block},
    network::{
        client::ClientPtr,
        peer::Peer,
        handler::Handler,
        message::Message,
    },
    config::Config
};

#[derive(PartialEq, Clone, Copy)]
enum State {
    Idle,
    Play,
    ExpectBlock,
}

macro_rules! check {
    ($ex:expr) => {
        if let Err(e) = $ex {
            log::error!("check: {}", e);
        }
    };
}


///
/// [Handler] is handling the incoming messages.
/// 
#[derive(Clone)]
pub struct DaemonHandler {
    state: Arc<Mutex<State>>,
    highlander: Arc<Mutex<Highlander>>,
    blockchain: Arc<Mutex<Blockchain>>,
}

impl DaemonHandler {
    async fn on_share(&self, peer: Peer<Self>, client: ClientPtr, data: Data) {
        self.blockchain.lock().await.add_to_cache(data.clone());

        let msg = Message::Share { data };
        check!(peer.broadcast(msg, Some(&client), None).await);

        let mut state = self.state.lock().await;

        if *state == State::Idle {
            *state = State::Play;

            let mut all_known = peer.all_known.lock().await.clone();
            all_known.insert(peer.key.public_key.clone());
            let mut hl = self.highlander.lock().await;
            hl.populate_roster(all_known.iter());
            let game = hl.create_game(&peer.key);
            hl.add_game(game.clone());

            if hl.is_filled() {
                let result = hl.evaluate(&peer.key);
                if result.winner == peer.key.public_key {
                    self.generate_new_block(&peer, result).await;
                }
                else {
                    log::error!("something really bad happend here");
                }
            }
            else {
                let msg = Message::Play { game };
                check!(peer.broadcast(msg, None, None).await);
            }
        }
    }

    async fn generate_new_block(&self, peer: &Peer<Self>, result: GameResult) {
        log::info!("create new block");

        let block = self.blockchain.lock().await.generate_new_block(result, &peer.key);
        block.validate();
        let msg = Message::AddBlock { block };
        check!(peer.broadcast(msg, None, None).await);
        *self.state.lock().await = State::Idle;
    }

    async fn on_game(&self, peer: Peer<Self>, client: ClientPtr, game: Game) {
        let mut hl = self.highlander.lock().await;
        hl.add_game(game.clone());

        let msg = Message::Play { game };
        check!(peer.broadcast(msg, Some(&client), None).await);

        if hl.is_filled() {
            let result = hl.evaluate(&peer.key);

            if result.winner == peer.key.public_key {
                self.generate_new_block(&peer, result).await;
            }
            else {
                *self.state.lock().await = State::ExpectBlock;
                log::info!("waiting for new block");
            }
        }
    }

    async fn on_new_block(&self, peer: Peer<Self>, client: ClientPtr, block: Block) {
        log::info!("got new block");
        self.blockchain.lock().await.add_new_block(block.clone());

        let msg = Message::AddBlock { block };
        check!(peer.broadcast(msg, Some(&client), None).await);
        *self.state.lock().await = State::Idle;
    }

    async fn on_highest_hash(&self, _peer: Peer<Self>, client: ClientPtr, hash: Vec<u8>, count: usize) {
        let (myhash, mycount) = self.blockchain.lock().await.highest_block();
        
        if myhash != hash && mycount < count {
            let msg = Message::RequestBlocks { from: myhash, to: hash };
            client.write_aes(&msg.to_bytes().unwrap()).await.unwrap();
        }
    }

    async fn on_request_blocks(&self, _peer: Peer<Self>, client: ClientPtr, from: Vec<u8>, to: Vec<u8>) {
        log::debug!("request blocks:\nfrom: {}\nto:   {}", hex::encode(&from), hex::encode(&to));

        let blocks = self.blockchain.lock().await.get_blocks(from, to).await.unwrap();
        let msg = Message::Blocks { blocks };
        client.write_aes(&msg.to_bytes().unwrap()).await.unwrap();
    }

    async fn on_blocks(&self, _peer: Peer<Self>, _client: ClientPtr, blocks: Vec<Block>) {
        let mut bc = self.blockchain.lock().await;

        for block in blocks {
            bc.add_new_block(block);
        }
    }
}

impl Handler for DaemonHandler {

    fn new(config: &Config) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::Idle)),
            highlander: Arc::new(Mutex::new(Highlander::new())),
            blockchain: Arc::new(Mutex::new(Blockchain::new(&config.folder))),
        }
    }
    
    fn init<'a>(&'a self, peer: Peer<Self>, client: ClientPtr) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
        async fn run(_self: &DaemonHandler, _peer: Peer<DaemonHandler>, _client: ClientPtr) where
        {
            let (hash, count) = _self.blockchain.lock().await.highest_block();
            let msg = Message::HighestBlock { hash, count };
            check!(_peer.broadcast(msg, None, None).await);
        }

        Box::pin(run(self, peer, client)) 
    }

    fn shutdown<'a>(&'a self, peer: Peer<Self>) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
        async fn run(_self: &DaemonHandler, _peer: Peer<DaemonHandler>) {
            _self.blockchain.lock().await.save_index();            
        }

        Box::pin(run(self, peer)) 
    }

    fn handle<'a>(&'a self, peer: Peer<Self>, client: ClientPtr, msg: Message) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
        async fn run(_self: &DaemonHandler, peer: Peer<DaemonHandler>, client: ClientPtr, msg: Message) {
            
            match msg {
                Message::Play { game } => {
                    _self.on_game(peer, client, game).await;
                }
                Message::Share { data } => {
                    _self.on_share(peer, client, data).await;
                }
                Message::AddBlock { block } => {
                    _self.on_new_block(peer, client, block).await;
                }
                Message::HighestBlock { hash, count } => {
                    _self.on_highest_hash(peer, client, hash, count).await;
                }
                Message::RequestBlocks { from, to } => {
                    _self.on_request_blocks(peer, client, from, to).await;
                }
                Message::Blocks { blocks } => {
                    _self.on_blocks(peer, client, blocks).await;
                }
                _ => {}
            }
        }

        Box::pin(run(self, peer, client, msg)) 
    }
}