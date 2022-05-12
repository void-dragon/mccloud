use std::{
    net::SocketAddr,
    sync::Arc,
    collections::{HashMap, HashSet},
    error::Error,
};

use rand::rngs::OsRng;
use serde::{Serialize, de::DeserializeOwned};
use tokio::{
    net::{
        TcpListener,
        TcpStream,
    },
    io,
    sync::Mutex, select,
};

use crate::{
    config::Config,
    key::{Key, PubKey},
    network::{
        envelope::Envelope,
        handler::Handler,
    },
};

use super::client::{ClientPtr, Client};


macro_rules! check {
    ($ex:expr) => {
        if let Err(e) = $ex {
            log::error!("{}", e);
        }
    };
}

#[derive(Clone)]
pub struct Peer<T> where T: Handler + ?Sized {
    pub key: Arc<Key>,
    pub config: Config,
    clients: Arc<Mutex<HashMap<SocketAddr, ClientPtr>>>,
    pub all_known: Arc<Mutex<HashSet<PubKey>>>,
    pub handler: Arc<T>,
}

impl<T> Peer<T> 
where 
    T: Handler + 'static,
    T::Msg: Serialize + DeserializeOwned + Send + Sync + std::fmt::Debug
{
    pub fn new(config: Config) -> Self {
        let handler = Arc::new(T::new(&config));

        Self {
            key: Arc::new(Key::new()),
            config: config,
            clients: Arc::new(Mutex::new(HashMap::new())),
            all_known: Arc::new(Mutex::new(HashSet::new())),
            handler,
        }
    }

    pub async fn listen(&self) -> io::Result<()> {
        log::info!("me {:?}", hex::encode(&self.key.public_key));

        let lst = TcpListener::bind((
            self.config.host.clone(),
            self.config.port
        )).await?;

        for cl in &self.config.clients {
            let stream = TcpStream::connect((cl.host.clone(), cl.port)).await?;
            let addr = stream.peer_addr()?;
            self.accept(stream, addr);
        }

        loop {
            select! {
                _ = tokio::signal::ctrl_c() => {
                    log::info!("begin shutdown");
                    self.handler.shutdown((*self).clone()).await;
                    break
                }
                Ok((socket, addr)) = lst.accept() => {
                    self.accept(socket, addr);
                }
            }
        }

        let clients = self.clients.lock().await;
        for cl in clients.values() {
            cl.shutdown().await;
        }

        Ok(())
    }

    fn accept(&self, stream: TcpStream, addr: SocketAddr) {
        let peer = (*self).clone();

        tokio::spawn(async move {
            let (reader, writer) = stream.into_split();
            let mut client = Arc::new(Client {
                pubkey: Vec::new(),
                ephemeral: k256::ecdh::EphemeralSecret::random(OsRng),
                addr,
                thin: false,
                writer: Mutex::new(writer),
                reader: Mutex::new(reader),
                shared: Vec::new(),
            });

            let shared = k256::EncodedPoint::from(client.ephemeral.public_key());
            let shared = shared.as_bytes();
            let greet = Envelope::<T::Msg>::Greeting { 
                id: peer.key.public_key.clone(),
                shared: shared.to_vec(),
                thin: peer.config.thin,
            }.to_bytes().unwrap();
            client.write(&greet).await.unwrap();

            let res: Envelope<T::Msg> = client.read().await.unwrap();
            if let Envelope::Greeting { id, thin, shared } = res {
                log::info!("id {}", hex::encode(&id));

                let shared = k256::PublicKey::from_sec1_bytes(&shared).unwrap();
                let shared = client.ephemeral.diffie_hellman(&shared).raw_secret_bytes().to_vec();
                log::debug!("shared {} {}", shared.len(), hex::encode(&shared));
                if let Some(cl) = Arc::get_mut(&mut client) {
                    cl.pubkey = id;
                    cl.shared = shared;
                    cl.thin = thin;
                }

                peer.clients.lock().await.insert(client.addr.clone(), client.clone());

                peer.handler.init(peer.clone(), client.clone()).await;

                if !client.thin {
                    let all_known = peer.all_known.lock().await.iter().cloned().collect();
                    let msg = Envelope::<T::Msg>::AllKnown { all_known }.to_bytes().unwrap();
                    check!(client.write_aes(&msg).await);
                }

                if !peer.config.thin {
                    let msg = Envelope::<T::Msg>::Announce { id: peer.key.public_key.clone() };
                    let msg = msg.to_bytes().unwrap();
                    check!(client.write_aes(&msg).await);
                }

                loop {
                    let env = client.read_aes().await;

                    match env {
                        Ok(env) => {
                            match env {
                                Envelope::AllKnown { all_known } => {
                                    peer.on_all_known(all_known).await;
                                }
                                Envelope::Announce { id } => {
                                    peer.on_announce(client.clone(), id).await;
                                }
                                Envelope::Remove { id } => {
                                    peer.on_remove(client.clone(), id).await;
                                }
                                Envelope::Message(msg) => {
                                    peer.handler.handle(peer.clone(), client.clone(), msg).await;
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            use tokio::io::ErrorKind;
                            if let Some(e) = e.downcast_ref::<tokio::io::Error>() {
                                match e.kind() {
                                    ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe => {}
                                    _ => {
                                        log::error!("{}", e);
                                    }
                                }
                            }
                            else {
                                log::error!("{}", e);
                            }
                            break
                        }
                    }
                }

                peer.disconnected(&client).await;
            }
        });
    }
    
    async fn disconnected(&self, client: &ClientPtr) {
        self.clients.lock().await.remove(&client.addr);

        log::debug!("disconnect {}", hex::encode(&client.pubkey));
        self.all_known.lock().await.remove(&client.pubkey);

        if !client.thin {
            check!(self.broadcast(Envelope::Remove{id: client.pubkey.clone()}).await);
        }
    }
    
    pub async fn broadcast(&self, msg: Envelope<T::Msg>) -> Result<(), Box<dyn Error>> {
        let data = msg.to_bytes()?;

        let clients = self.clients.lock().await;

        for cl in clients.values() {
            cl.write_aes(&data).await?;
        }

        Ok(())
    }

    pub async fn broadcast_except(&self, msg: Envelope<T::Msg>, ex: &ClientPtr) -> Result<(), Box<dyn Error>> {
        let data = msg.to_bytes()?;

        let clients = self.clients.lock().await;

        for cl in clients.values() {
            if cl.addr != ex.addr {
                cl.write_aes(&data).await?;
            }
        }

        Ok(())
    }

    async fn on_announce(&self, client: ClientPtr, id: PubKey) {
        log::debug!("announce {}", hex::encode(&id));
        let already_known = self.all_known.lock().await.insert(id.clone());
        if already_known {
            log::debug!("propergate announce {}", hex::encode(&id));
            check!(self.broadcast_except(Envelope::Announce { id }, &client).await);
        }
    }

    async fn on_all_known(&self, all_known: Vec<PubKey>) {
        log::debug!("allknown");
        let mut my_known = self.all_known.lock().await;
        my_known.extend(all_known);
    }

    async fn on_remove(&self, client: ClientPtr, id: PubKey) {
        log::debug!("remove {}", hex::encode(&id));
        let already_known = self.all_known.lock().await.remove(&id);
        if already_known {
            log::debug!("propergate remove {}", hex::encode(&id));
            check!(self.broadcast_except(Envelope::Announce { id }, &client).await);
        }
    }
}