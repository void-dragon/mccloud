use std::{sync::Arc, pin::Pin, future::Future};

use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::Mutex;

use crate::{
    highlander::{Highlander, Game},
    blockchain::{Blockchain, Data, Block},
    network::{
        peer::{ClientPtr, Peer},
        handler::Handler,
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
            log::error!("{}", e);
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

            let msg = Messages::Play { game };
            check!(peer.broadcast(msg.into()).await);
        }
    }

    async fn on_game(&self, peer: Peer<Self>, client: ClientPtr, game: Game) {
        let mut hl = self.highlander.lock().await;
        hl.add_game(game.clone());

        let msg = Messages::Play { game };
        check!(peer.broadcast_except(msg.into(), &client).await);

        if hl.is_filled() {
            let result = hl.evaluate(&peer.key);

            if result.winner == peer.key.public_key {
                log::info!("create new block");
                let block = self.blockchain.lock().await.generate_new_block(result, &peer.key);
                let msg = Messages::ShareBlock { block };
                check!(peer.broadcast(msg.into()).await);
                *self.state.lock().await = State::Idle;
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

        let msg = Messages::ShareBlock { block };
        check!(peer.broadcast_except(msg.into(), &client).await);
        *self.state.lock().await = State::Idle;
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
                Messages::ShareBlock { block } => {
                    _self.on_new_block(peer, client, block).await;
                }
                Messages::UserData(_) => {}
            }
        }

        Box::pin(run(self, peer, client, msg)) 
    }
}