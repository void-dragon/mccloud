use std::{sync::Arc, net::SocketAddr};

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use rand::{rngs::OsRng, RngCore};
use tokio::{
    net::{
        tcp::{OwnedWriteHalf, OwnedReadHalf}, TcpStream
    },
    io::{AsyncWriteExt, AsyncReadExt},
    sync::Mutex
};

use crate::key::PubKey;

use super::message::Message;

pub type AesCbcEnc = cbc::Encryptor<aes::Aes256>;
pub type AesCbcDec = cbc::Decryptor<aes::Aes256>;

pub struct Client {
    pub pubkey: PubKey,
    pub ephemeral: k256::ecdh::EphemeralSecret,
    pub thin: bool,
    pub addr: SocketAddr,
    pub writer: Mutex<OwnedWriteHalf>,
    pub reader: Mutex<OwnedReadHalf>,
    pub shared: Vec<u8>,
}

impl Client {
    pub fn new(stream: TcpStream, addr: SocketAddr) -> Arc<Self> {
        let (reader, writer) = stream.into_split();
        Arc::new(Client {
            pubkey: Vec::new(),
            ephemeral: k256::ecdh::EphemeralSecret::random(OsRng),
            addr,
            thin: false,
            writer: Mutex::new(writer),
            reader: Mutex::new(reader),
            shared: Vec::new(),
        })
    }

    pub async fn write_aes(&self, data: &Vec<u8>) -> Result<(), anyhow::Error> {
        let mut iv = [0u8; 16];
        OsRng.fill_bytes(&mut iv);
        let enc = AesCbcEnc::new_from_slices(&self.shared, &iv).unwrap();
        let encrypted = enc.encrypt_padded_vec_mut::<Pkcs7>(&data);

        let size = (encrypted.len() as u32).to_be_bytes();

        let mut writer = self.writer.lock().await;
        writer.write(&size).await?;
        writer.write(&iv).await?;
        writer.write_all(&encrypted).await?;

        Ok(())
    }

    pub async fn write(&self, data: &Vec<u8>) -> Result<(), anyhow::Error> {
        let size = (data.len() as u32).to_be_bytes();

        let mut writer = self.writer.lock().await;
        writer.write(&size).await?;
        writer.write_all(&data).await?;

        Ok(())
    }

    pub async fn read_aes(&self) -> Result<Message, anyhow::Error> {
        let mut reader = self.reader.lock().await;
        let mut size_bytes = [0; 4];
        reader.read_exact(&mut size_bytes).await?;
        let size = u32::from_be_bytes(size_bytes) as usize;
        let mut iv = [0u8; 16];
        reader.read_exact(&mut iv).await?;
        let mut buffer = vec![0u8; size];
        reader.read_exact(&mut buffer).await?;

        let dec = AesCbcDec::new_from_slices(&self.shared, &iv).unwrap();
        let data: Vec<u8> = dec.decrypt_padded_vec_mut::<Pkcs7>(&buffer).unwrap();

        Ok(rmp_serde::from_slice(&data)?)
    }

    pub async fn read(&self) -> Result<Message, anyhow::Error> {
        let mut reader = self.reader.lock().await;
        let mut size_bytes = [0; 4];
        reader.read_exact(&mut size_bytes).await?;
        let size = u32::from_be_bytes(size_bytes) as usize;
        let mut buffer = vec![0u8; size];
        reader.read_exact(&mut buffer).await?;

        Ok(rmp_serde::from_slice(&buffer)?)
    }

    pub async fn shutdown(&self) {
        let mut w = self.writer.lock().await;
        if let Err(e) = w.shutdown().await {
            log::error!("shutdown: {}", e);
        }
    }
}

pub type ClientPtr = Arc<Client>;