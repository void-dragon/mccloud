use std::{sync::Arc, net::SocketAddr};

use openssl::symm;
use serde::{Serialize, de::DeserializeOwned};
use tokio::{
    net::{
        tcp::{OwnedWriteHalf, OwnedReadHalf}
    },
    io::{AsyncWriteExt, AsyncReadExt},
    sync::Mutex
};

use crate::key::PubKey;

use super::envelope::Envelope;


pub struct Client {
    pub pubkey: PubKey,
    pub thin: bool,
    pub addr: SocketAddr,
    pub writer: Mutex<OwnedWriteHalf>,
    pub reader: Mutex<OwnedReadHalf>,
    pub shared: Vec<u8>,
}

impl Client {
    pub async fn write_aes<T: Serialize>(&self, msg: Envelope<T>) -> Result<(), anyhow::Error> {
        let data = rmp_serde::to_vec_named(&msg)?;
        let cipher = symm::Cipher::aes_256_ctr();
        let encrypted = symm::encrypt(cipher, &self.shared, None, &data)?;

        let size = (encrypted.len() as u32).to_be_bytes();

        let mut writer = self.writer.lock().await;
        writer.write(&size).await?;
        writer.write_all(&encrypted).await?;

        Ok(())
    }

    pub async fn write<T: Serialize>(&self, msg: Envelope<T>) -> Result<(), anyhow::Error> {
        let data = rmp_serde::to_vec_named(&msg)?;
        let size = (data.len() as u32).to_be_bytes();

        let mut writer = self.writer.lock().await;
        writer.write(&size).await?;
        writer.write_all(&data).await?;

        Ok(())
    }

    pub async fn read_aes<T: DeserializeOwned>(&self) -> Result<Envelope<T>, anyhow::Error> {
        let mut reader = self.reader.lock().await;
        let mut size_bytes = [0; 4];
        reader.read_exact(&mut size_bytes).await?;
        let size = u32::from_be_bytes(size_bytes) as usize;
        let mut buffer = vec![0u8; size];
        reader.read_exact(&mut buffer).await?;

        let cipher = symm::Cipher::aes_256_ctr();
        let data = symm::decrypt(cipher, &self.shared, None, &buffer)?;

        Ok(rmp_serde::from_slice(&data)?)
    }

    pub async fn read<T: DeserializeOwned>(&self) -> Result<Envelope<T>, anyhow::Error> {
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
        w.shutdown().await.unwrap();
    }
}

pub type ClientPtr = Arc<Client>;