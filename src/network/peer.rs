use std::{
    net::SocketAddr,
    sync::Arc,
    collections::{HashMap, HashSet},
    error::Error, future::Future, pin::Pin
};

use openssl::{rand::rand_bytes, symm};
use serde::{Serialize, de::DeserializeOwned};
use tokio::{
    net::{
        TcpListener,
        TcpStream,
        tcp::OwnedWriteHalf
    },
    io::{self, AsyncWriteExt},
    sync::Mutex,
};

use crate::{
    config::Config,
    key::{Key, PubKey},
    network::envelope::Envelope,
};

pub struct Client {
    pubkey: PubKey,
    addr: SocketAddr,
    writer: Mutex<OwnedWriteHalf>,
    enc_aes_key: [u8; 32],
    enc_iv: [u8; 32],
    dec_aes_key: [u8; 32],
    dec_iv: [u8; 32],
}

pub type ClientPtr = Arc<Client>;

macro_rules! check {
    ($ex:expr) => {
        if let Err(e) = $ex {
            log::error!("{}", e);
        }
    };
}

pub trait Handler: Send + Sync {
    type Msg;

    fn new() -> Self;

    fn handle<'a>( &'a self, peer: Peer<Self>, client: ClientPtr, msg: Self::Msg) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a;
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
    T: Handler + Clone + 'static,
    T::Msg: Serialize + DeserializeOwned + Send + Sync
{
    pub fn new(config: Config) -> Self {
        Self {
            key: Arc::new(Key::new().unwrap()),
            config: config,
            clients: Arc::new(Mutex::new(HashMap::new())),
            all_known: Arc::new(Mutex::new(HashSet::new())),
            handler: Arc::new(T::new()),
        }
    }

    pub async fn listen(&self) -> io::Result<()> {
        log::info!("me {}", hex::encode(&self.key.public_key));

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
            let (socket, addr) = lst.accept().await?;
            self.accept(socket, addr);
        }
    }

    fn accept(&self, mut stream: TcpStream, addr: SocketAddr) {
        let peer = (*self).clone();

        tokio::spawn(async move {
            let greet = Envelope::<T::Msg>::Greeting { id: peer.key.public_key.clone() };
            greet.write(&mut stream).await.unwrap();

            let res = Envelope::<T::Msg>::read(&mut stream).await.unwrap();
            if let Envelope::Greeting { id } = res {
                log::info!("id: {}", hex::encode(&id));
                let mut enc_aes = [0u8; 32];
                let mut enc_iv = [0u8; 32];
                rand_bytes(&mut enc_aes).unwrap();
                rand_bytes(&mut enc_iv).unwrap();

                let key_share = Envelope::<T::Msg>::AesKey { aes: enc_aes.clone(), iv: enc_iv.clone() };
                key_share.write_ec(&mut stream, &id).await.unwrap();

                let res = Envelope::<T::Msg>::read_ec(&mut stream, &peer.key).await.unwrap();
                if let Envelope::AesKey { aes, iv } = res {

                    let (mut reader, writer) = stream.into_split();
                    let client = Arc::new(Client {
                        pubkey: id,
                        addr,
                        writer: Mutex::new(writer),
                        enc_aes_key: enc_aes,
                        enc_iv: iv,
                        dec_aes_key: aes,
                        dec_iv: iv,
                    });

                    peer.clients.lock().await.insert(client.addr.clone(), client.clone());

                    loop {
                        let env = Envelope::read_aes(
                            &mut reader, 
                            &client.dec_aes_key, 
                            &client.dec_iv
                        ).await;

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
            }
        });
    }
    
    async fn disconnected(&self, client: &ClientPtr) {
        self.clients.lock().await.remove(&client.addr);

        log::debug!("disconnect {}", hex::encode(&client.pubkey));
        self.all_known.lock().await.remove(&client.pubkey);

        check!(self.broadcast(Envelope::Remove{id: client.pubkey.clone()}).await);
    }
    
    pub async fn broadcast(&self, msg: Envelope<T::Msg>) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(&msg)?;
        let cipher = symm::Cipher::aes_256_cbc();

        let clients = self.clients.lock().await;

        for cl in clients.values() {
            let encrypted = symm::encrypt(cipher, &cl.enc_aes_key, Some(&cl.enc_iv), &data)?;

            let size = (encrypted.len() as u32).to_be_bytes();

            let mut writer = cl.writer.lock().await;
            writer.write(&size).await?;
            writer.write_all(&encrypted).await?;
        }

        Ok(())
    }

    pub async fn broadcast_except(&self, msg: Envelope<T::Msg>, ex: &ClientPtr) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(&msg)?;
        let cipher = symm::Cipher::aes_256_cbc();

        let clients = self.clients.lock().await;

        for cl in clients.values() {
            if cl.addr != ex.addr {
                let encrypted = symm::encrypt(cipher, &cl.enc_aes_key, Some(&cl.enc_iv), &data)?;
                let size = (encrypted.len() as u32).to_be_bytes();

                let mut stream = cl.writer.lock().await;
                stream.write(&size).await?;
                stream.write_all(&encrypted).await?;
            }
        }

        Ok(())
    }

    async fn on_announce(&self, client: ClientPtr, id: PubKey) {
        let already_known = self.all_known.lock().await.insert(id.clone());
        if already_known {
            log::debug!("propergate announce {}", hex::encode(&id));
            check!(self.broadcast_except(Envelope::Announce { id }, &client).await);
        }
    }

    async fn on_all_known(&self, all_known: Vec<PubKey>) {
        let mut my_known = self.all_known.lock().await;
        my_known.extend(all_known);
    }

    async fn on_remove(&self, client: ClientPtr, id: PubKey) {
        let already_known = self.all_known.lock().await.remove(&id);
        if already_known {
            log::debug!("propergate remove {}", hex::encode(&id));
            check!(self.broadcast_except(Envelope::Announce { id }, &client).await);
        }
    }
}