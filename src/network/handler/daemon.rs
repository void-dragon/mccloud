use std::{sync::Arc, pin::Pin, future::Future};

use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::Mutex;

use crate::{
    highlander::{Highlander, Game, GameResult},
    blockchain::{Blockchain, Data, Block},
    network::{
        client::ClientPtr,
        peer::Peer,
        handler::Handler, envelope::Envelope,
    },
    messages::Messages, config::Config
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

pub trait UserDataHandler: Send + Sync + Clone {
    type UserData;

    fn new() -> Self;

    fn handle<'a>(&'a self, data: Self::UserData) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a;
}

///
/// [Handler] is handling the incoming messages.
/// 
#[derive(Clone)]
pub struct DaemonHandler<T> {
    state: Arc<Mutex<State>>,
    highlander: Arc<Mutex<Highlander>>,
    blockchain: Arc<Mutex<Blockchain>>,
    user_data_handler: Arc<T>,
}

impl<T> DaemonHandler<T>
where 
    T: UserDataHandler + 'static,
    T::UserData: Serialize + DeserializeOwned + Send + Sync + std::fmt::Debug
{
    async fn on_share(&self, peer: Peer<Self>, client: ClientPtr, data: Data) {
        self.blockchain.lock().await.add_to_cache(data.clone());

        let msg = Messages::Share { data };
        check!(peer.broadcast_except(msg.into(), &client).await);

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
                let msg = Messages::Play { game };
                check!(peer.broadcast(msg.into()).await);
            }
        }
    }

    async fn generate_new_block(&self, peer: &Peer<Self>, result: GameResult) {
        log::info!("create new block");

        let block = self.blockchain.lock().await.generate_new_block(result, &peer.key);
        block.validate();
        let msg = Messages::AddBlock { block };
        check!(peer.broadcast(msg.into()).await);
        *self.state.lock().await = State::Idle;
    }

    async fn on_game(&self, peer: Peer<Self>, client: ClientPtr, game: Game) {
        let mut hl = self.highlander.lock().await;
        hl.add_game(game.clone());

        let msg = Messages::Play { game };
        check!(peer.broadcast_except(msg.into(), &client).await);

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

        let msg = Messages::AddBlock { block };
        check!(peer.broadcast_except(msg.into(), &client).await);
        *self.state.lock().await = State::Idle;
    }

    async fn on_highest_hash(&self, _peer: Peer<Self>, client: ClientPtr, hash: Vec<u8>, count: usize) {
        let (myhash, mycount) = self.blockchain.lock().await.highest_block();
        
        if myhash != hash && mycount < count {
            let msg = Messages::<T::UserData>::RequestBlocks { from: myhash, to: hash };
            let env = Envelope::from(msg);
            client.write_aes(&env.to_bytes().unwrap()).await.unwrap();
        }
    }

    async fn on_request_blocks(&self, _peer: Peer<Self>, client: ClientPtr, from: Vec<u8>, to: Vec<u8>) {
        log::debug!("request blocks:\nfrom: {}\nto:   {}", hex::encode(&from), hex::encode(&to));

        let blocks = self.blockchain.lock().await.get_blocks(from, to).await.unwrap();
        let msg = Messages::<T::UserData>::Blocks { blocks };
        let env = Envelope::from(msg);
        client.write_aes(&env.to_bytes().unwrap()).await.unwrap();
    }

    async fn on_blocks(&self, _peer: Peer<Self>, _client: ClientPtr, blocks: Vec<Block>) {
        let mut bc = self.blockchain.lock().await;

        for block in blocks {
            bc.add_new_block(block);
        }
    }
}

impl<T> Handler for DaemonHandler<T>
where
    T: UserDataHandler + 'static,
    T::UserData: Serialize + DeserializeOwned + Send + Sync + std::fmt::Debug
{
    type Msg = Messages<T::UserData>;

    fn new(config: &Config) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::Idle)),
            highlander: Arc::new(Mutex::new(Highlander::new())),
            blockchain: Arc::new(Mutex::new(Blockchain::new(&config.folder))),
            user_data_handler: Arc::new(T::new()),
        }
    }
    
    fn init<'a>(&'a self, peer: Peer<Self>, client: ClientPtr) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
        async fn run<T>(_self: &DaemonHandler<T>, _peer: Peer<DaemonHandler<T>>, _client: ClientPtr) where
            T: UserDataHandler + 'static,
            T::UserData: Serialize + DeserializeOwned + Send + Sync + std::fmt::Debug
        {
            let (hash, count) = _self.blockchain.lock().await.highest_block();
            let msg = Messages::<T::UserData>::HighestBlock { hash, count };
            check!(_peer.broadcast(msg.into()).await);
        }

        Box::pin(run(self, peer, client)) 
    }

    fn shutdown<'a>(&'a self, peer: Peer<Self>) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
        async fn run<T: UserDataHandler>(_self: &DaemonHandler<T>, _peer: Peer<DaemonHandler<T>>) where
            T: UserDataHandler + 'static,
            T::UserData: Serialize + DeserializeOwned + Send + Sync + std::fmt::Debug
        {
            _self.blockchain.lock().await.save_index();            
        }

        Box::pin(run(self, peer)) 
    }

    fn handle<'a>(&'a self, peer: Peer<Self>, client: ClientPtr, msg: Self::Msg) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a {
        async fn run<T: UserDataHandler>(_self: &DaemonHandler<T>, peer: Peer<DaemonHandler<T>>, client: ClientPtr, msg: Messages<T::UserData>) 
        where
            T: UserDataHandler + 'static,
            T::UserData: Serialize + DeserializeOwned + Send + Sync + std::fmt::Debug
        {
            
            match msg {
                Messages::Play { game } => {
                    _self.on_game(peer, client, game).await;
                }
                Messages::Share { data } => {
                    _self.on_share(peer, client, data).await;
                }
                Messages::AddBlock { block } => {
                    _self.on_new_block(peer, client, block).await;
                }
                Messages::UserData(data) => {
                    _self.user_data_handler.handle(data).await;
                }
                Messages::HighestBlock { hash, count } => {
                    _self.on_highest_hash(peer, client, hash, count).await;
                }
                Messages::RequestBlocks { from, to } => {
                    _self.on_request_blocks(peer, client, from, to).await;
                }
                Messages::Blocks { blocks } => {
                    _self.on_blocks(peer, client, blocks).await;
                }
            }
        }

        Box::pin(run(self, peer, client, msg)) 
    }
}