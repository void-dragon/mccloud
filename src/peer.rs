use std::{
    net::{TcpListener, TcpStream, SocketAddr},
    io::Write,
    sync::{Arc, Mutex},
    error::Error,
    collections::{HashSet, HashMap},
};

use crate::{
    config::Config,
    messages::Messages,
    key::{Key, PubKey},
    highlander::{Highlander, Game},
    blockchain::{Data, Blockchain, Block}
};


#[derive(PartialEq, Clone, Copy)]
enum State {
    Idle,
    Play,
    ExpectBlock,
}

struct Client {
    id: Option<PubKey>,
    addr: SocketAddr,
    stream: TcpStream
}

type ClientPtr = Arc<Mutex<Client>>;

macro_rules! check {
    ($ex:expr) => {
        if let Err(e) = $ex {
            log::error!("{}", e);
        }
    };
}

#[derive(Clone)]
pub struct Peer {
    config: Config,
    state: Arc<Mutex<State>>,
    key: Arc<Key>,
    highlander: Arc<Mutex<Highlander>>,
    blockchain: Arc<Mutex<Blockchain>>,
    clients: Arc<Mutex<HashMap<SocketAddr, ClientPtr>>>,
    all_known: Arc<Mutex<HashSet<PubKey>>>,
}

impl Peer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(State::Idle)),
            key: Arc::new(Key::new().unwrap()),
            clients: Arc::new(Mutex::new(HashMap::new())),
            highlander: Arc::new(Mutex::new(Highlander::new())),
            blockchain: Arc::new(Mutex::new(Blockchain::new())),
            all_known: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn listen(&self) {
        log::info!("id {}", hex::encode(&self.key.public_key));
        log::info!("liste on {}:{}", self.config.host, self.config.port);

        // self.all_known.lock().unwrap().insert(self.key.public_key.clone());

        for cl in &self.config.clients {
            match TcpStream::connect((cl.host.clone(), cl.port)) {
                Ok(stream) => {
                    let addr = stream.peer_addr().unwrap();
                    self.accept(stream, addr);
                }
                Err(e) => {
                    log::warn!("{}", e);
                }
            }
        }

        let lst = TcpListener::bind((self.config.host.clone(), self.config.port)).unwrap();

        while let Ok((stream, addr)) = lst.accept() {
            self.accept(stream, addr);
        }
    }

    fn accept(&self, stream: TcpStream, addr: SocketAddr) {
        let mut reader = stream.try_clone().unwrap();
        let peer = self.clone();
        let id = self.key.public_key.clone();
        let all_known = self.all_known.lock().unwrap().iter().cloned().collect();
        let client = Arc::new(Mutex::new(Client { id: None, addr: addr.clone(), stream }));

        self.clients.lock().unwrap().insert(addr, client.clone());

        std::thread::spawn(move || {
            Messages::Greeting { 
                id: id.clone(),
                all_known: all_known,
            }.write(&mut reader).unwrap();
            Messages::Announce { id: id }.write(&mut reader).unwrap();

            loop {
                match Messages::read(&mut reader) {
                    Ok(msg) => {
                        // log::info!("{:?}", msg);

                        match msg {
                            Messages::Greeting { id , all_known} => { 
                                peer.on_greeting(&client, id, all_known);
                            }
                            Messages::Announce { id } => {
                                peer.on_announce(&client, id);
                            }
                            Messages::Remove { id } => {
                                peer.on_remove(id);
                            }
                            Messages::Share { data } => {
                                peer.on_share(&client, data);
                            }
                            Messages::Play {game} => {
                                peer.on_game(&client, game);
                            }
                            Messages::ShareBlock { block } => {
                                peer.on_new_block(&client, block);
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("{}", e);
                        break
                    }
                }
            }

            peer.disconnected(&client);
        });

    }

    fn disconnected(&self, client: &ClientPtr) {
        let cl = client.lock().unwrap();

        self.clients.lock().unwrap().remove(&cl.addr);

        if let Some(ref id) = cl.id {
            log::debug!("disconnect {}", hex::encode(id));
            self.all_known.lock().unwrap().remove(id);

            check!(self.broadcast(Messages::Remove{id: id.clone()}));
        }
    }

    fn broadcast(&self, msg: Messages) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(&msg)?;
        let size = (data.len() as u32).to_be_bytes();

        let clients = self.clients.lock().unwrap();

        for cl in clients.values() {
            let mut cl = cl.lock().unwrap();

            if cl.id.is_some() {
                cl.stream.write(&size)?;
                cl.stream.write_all(&data)?;
            }
        }

        Ok(())
    }

    fn broadcast_except(&self, msg: Messages, ex: &ClientPtr) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(&msg)?;
        let size = (data.len() as u32).to_be_bytes();

        let ex_addr = ex.lock().unwrap().addr;
        let clients = self.clients.lock().unwrap();

        for cl in clients.values() {
            let mut cl = cl.lock().unwrap();
            if cl.id.is_some() && cl.addr != ex_addr {
                cl.stream.write(&size)?;
                cl.stream.write_all(&data)?;
            }
        }

        Ok(())
    }

    fn on_greeting(&self, client: &ClientPtr, id: PubKey, all_known: Vec<PubKey>) {
        log::debug!("greeting from {}", hex::encode(&id));
        client.lock().unwrap().id = Some(id);
        let mut my_known = self.all_known.lock().unwrap();
        for key in all_known {
            my_known.insert(key);
        }
    }

    fn on_announce(&self, client: &ClientPtr, id: PubKey) {
        log::debug!("announce {}", hex::encode(&id));
        if self.all_known.lock().unwrap().insert(id.clone()) {
            log::debug!("propergate announce {}", hex::encode(&id));
            check!(self.broadcast_except(Messages::Announce { id }, client));
        }
    }

    fn on_remove(&self, id: PubKey) {
        log::debug!("remove {}", hex::encode(&id));
        if self.all_known.lock().unwrap().remove(&id) {
            log::debug!("propergate remove {}", hex::encode(&id));
            check!(self.broadcast(Messages::Remove { id }));
        }
    }

    fn on_share(&self, client: &ClientPtr, data: Data) {
        self.blockchain.lock().unwrap().add_to_cache(data.clone());

        check!(self.broadcast_except(Messages::Share { data }, &client));

        let mut state = self.state.lock().unwrap();

        if *state == State::Idle {
            *state = State::Play;

            let mut all_known = self.all_known.lock().unwrap().clone();
            all_known.insert(self.key.public_key.clone());
            let mut hl = self.highlander.lock().unwrap();
            hl.populate_roster(all_known.iter());
            let game = hl.create_game(&self.key);
            hl.add_game(game.clone());

            check!(self.broadcast(Messages::Play { game }));
        }
    }

    fn on_game(&self, client: &ClientPtr, game: Game) {
        let mut hl = self.highlander.lock().unwrap();
        hl.add_game(game.clone());

        check!(self.broadcast_except(Messages::Play { game }, client));

        if hl.is_filled() {
            let result = hl.evaluate(&self.key);

            if result.winner == self.key.public_key {
                let block = self.blockchain.lock().unwrap().generate_new_block(result, &self.key);
                let msg = Messages::ShareBlock { block };
                check!(self.broadcast(msg));
                *self.state.lock().unwrap() = State::Idle;
            }
            else {
                *self.state.lock().unwrap() = State::ExpectBlock;
                log::info!("waiting for new block");
            }
        }
    }

    fn on_new_block(&self, client: &ClientPtr, block: Block) {
        log::info!("got new block");
        self.blockchain.lock().unwrap().add_new_block(block.clone());

        let msg = Messages::ShareBlock { block };
        check!(self.broadcast_except(msg, client));
        *self.state.lock().unwrap() = State::Idle;
    }
}