use std::{
    net::SocketAddr,
    sync::Arc,
    collections::{HashMap, HashSet},
    error::Error, time::Duration,
};

use tokio::{
    net::{
        TcpListener,
        TcpStream,
    },
    io,
    sync::{Mutex, Notify}, select,
};

use crate::{
    config::Config,
    key::{Key, PubKey},
    network::{
        message::Message,
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
    close: Arc<Notify>,
    clients: Arc<Mutex<HashMap<SocketAddr, ClientPtr>>>,
    pub all_known: Arc<Mutex<HashSet<PubKey>>>,
    pub handler: Arc<T>,
}

impl<T> Peer<T> 
where 
    T: Handler + 'static,
{
    pub fn new(config: Config) -> Self {
        let handler = Arc::new(T::new(&config));

        Self {
            key: Arc::new(Key::new()),
            config: config,
            close: Arc::new(Notify::new()),
            clients: Arc::new(Mutex::new(HashMap::new())),
            all_known: Arc::new(Mutex::new(HashSet::new())),
            handler,
        }
    }

    pub fn shutdown(&self) {
        self.close.notify_one();
    }

    pub async fn listen(&self) -> io::Result<()> {
        log::info!("me {}", hex::encode(&self.key.public_key));

        self.all_known.lock().await.insert(self.key.public_key.clone());

        let lst = TcpListener::bind((
            self.config.host.clone(),
            self.config.port
        )).await?;

        for cl in &self.config.clients {
            let stream = TcpStream::connect((cl.host.clone(), cl.port)).await?;
            let addr = stream.peer_addr()?;
            self.accept(stream, addr, cl.reconnect);
        }

        loop {
            select! {
                _ = self.close.notified() => {
                    break
                }
                _ = tokio::signal::ctrl_c() => {
                    break
                }
                Ok((socket, addr)) = lst.accept() => {
                    self.accept(socket, addr, false);
                }
            }
        }

        log::info!("begin shutdown");
        self.handler.shutdown((*self).clone()).await;

        let clients = self.clients.lock().await;
        for cl in clients.values() {
            cl.shutdown().await;
        }

        Ok(())
    }

    fn accept(&self, stream: TcpStream, addr: SocketAddr, reconnect: bool) {
        let peer = (*self).clone();

        tokio::spawn(async move {
            let mut client = Client::new(stream, addr);

            let shared = k256::EncodedPoint::from(client.ephemeral.public_key());
            let shared = shared.as_bytes();
            let greet = Message::Greeting { 
                id: peer.key.public_key.clone(),
                shared: shared.to_vec(),
                thin: peer.config.thin,
            }.to_bytes().unwrap();
            client.write(&greet).await.unwrap();

            let res = client.read().await.unwrap();
            if let Message::Greeting { id, thin, shared } = res {
                log::info!("id {}", hex::encode(&id));

                let shared = k256::PublicKey::from_sec1_bytes(&shared).unwrap();
                let shared = client.ephemeral.diffie_hellman(&shared).raw_secret_bytes().to_vec();
                if let Some(cl) = Arc::get_mut(&mut client) {
                    cl.pubkey = id;
                    cl.shared = shared;
                    cl.thin = thin;
                }

                peer.clients.lock().await.insert(client.addr.clone(), client.clone());

                peer.handler.init(peer.clone(), client.clone()).await;

                if !client.thin  && !peer.config.thin {
                    let all_known = peer.all_known.lock().await
                        .iter()
                        .map(|n| serde_bytes::ByteBuf::from(n.clone()))
                        .collect();
                    let msg = Message::AllKnown { all_known }.to_bytes().unwrap();
                    check!(client.write_aes(&msg).await);

                    let msg = Message::Announce { id: peer.key.public_key.clone() };
                    let msg = msg.to_bytes().unwrap();
                    check!(client.write_aes(&msg).await);
                }

                loop {
                    let env = client.read_aes().await;

                    match env {
                        Ok(env) => {
                            peer.handle_envelope(&client, env).await;
                        }
                        Err(e) => {
                            use tokio::io::ErrorKind;
                            if let Some(e) = e.downcast_ref::<tokio::io::Error>() {
                                match e.kind() {
                                    ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe => {}
                                    _ => {
                                        log::error!("read-aes: {}", e);
                                    }
                                }
                            }
                            else {
                                log::error!("read-aes: {}", e);
                            }
                            break
                        }
                    }
                }

                peer.disconnected(&client).await;

                if reconnect {
                    peer.try_reconnect(client.addr).await;
                }
            }
        });
    }

    async fn handle_envelope(&self, client: &ClientPtr, env: Message) {
        match env {
            Message::AllKnown { all_known } => {
                self.on_all_known(all_known).await;
            }
            Message::Announce { id } => {
                self.on_announce(client.clone(), id).await;
            }
            Message::Remove { id } => {
                self.on_remove(client.clone(), id).await;
            }
            _ => {
                self.handler.handle(self.clone(), client.clone(), env).await;
            }
        }
    }
    
    async fn disconnected(&self, client: &ClientPtr) {
        self.clients.lock().await.remove(&client.addr);

        log::debug!("disconnect {}", hex::encode(&client.pubkey));
        self.all_known.lock().await.remove(&client.pubkey);

        if !client.thin {
            check!(self.broadcast(Message::Remove{id: client.pubkey.clone()}, None, None).await);
        }
    }

    async fn try_reconnect(&self, addr: SocketAddr) {
        let peer = (*self).clone();

        tokio::spawn(async move {
            loop {
                log::debug!("try reconnect to {:?}", addr);

                if let Ok(stream) = TcpStream::connect(addr).await {
                    let addr = stream.peer_addr().unwrap();
                    peer.accept(stream, addr, true);
                    break;
                }
                else {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                }
            }
        });
    }

    pub async fn broadcast(&self, msg: Message, ex: Option<&ClientPtr>, thin: Option<bool>) -> Result<(), Box<dyn Error>> {
        let data = msg.to_bytes()?;

        let thin = thin.unwrap_or(false);
        let clients = self.clients.lock().await;

        if let Some(ex) = ex {
            for cl in clients.values() {
                if cl.addr != ex.addr && thin || !cl.thin {
                    cl.write_aes(&data).await?;
                }
            }
        }
        else {
            for cl in clients.values() {
                if thin || !cl.thin {
                    cl.write_aes(&data).await?;
                }
            }
        }

        Ok(())
    }

    async fn on_announce(&self, client: ClientPtr, id: PubKey) {
        log::debug!("announce {}", hex::encode(&id));
        let already_known = self.all_known.lock().await.insert(id.clone());
        if already_known {
            log::debug!("propergate announce {}", hex::encode(&id));
            check!(self.broadcast(Message::Announce { id }, Some(&client), None).await);
        }
    }

    async fn on_all_known(&self, all_known: Vec<serde_bytes::ByteBuf>) {
        log::debug!("allknown");
        let mut my_known = self.all_known.lock().await;
        for buf in all_known {
            my_known.insert(buf.into_vec());
        }
    }

    async fn on_remove(&self, client: ClientPtr, id: PubKey) {
        log::debug!("remove {}", hex::encode(&id));
        let already_known = self.all_known.lock().await.remove(&id);
        if already_known {
            log::debug!("propergate remove {}", hex::encode(&id));
            check!(self.broadcast(Message::Announce { id }, Some(&client), None).await);
        }
    }
}