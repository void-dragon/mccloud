use std::error::Error;

use openssl::symm;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use tokio::io::{AsyncWriteExt, AsyncReadExt};

use crate::key::PubKey;

#[derive(Serialize, Deserialize, Debug)]
pub enum Envelope<T> {
    Greeting {
        #[serde(with="serde_bytes")]
        id: PubKey,
        thin: bool
    },
    AesKey {
        #[serde(with="serde_bytes")]
        aes: Vec<u8>, 
        #[serde(with="serde_bytes")]
        iv: Vec<u8>,
        #[serde(with="serde_bytes")]
        sign: Vec<u8>
    },
    AllKnown { all_known: Vec<PubKey>},
    Announce {
        #[serde(with="serde_bytes")]
        id: PubKey
    },
    Remove {
        #[serde(with="serde_bytes")]
        id: PubKey
    },
    Message(T),
}

impl<M> Envelope<M> 
where
    M: Serialize + DeserializeOwned
{
    pub async fn write_aes<T: AsyncWriteExt + Unpin>(&self, writer: &mut T, aes: &[u8]) -> Result<(), Box<dyn Error>> {
        let data = rmp_serde::to_vec_named(&self)?;
        let cipher = symm::Cipher::aes_256_ctr();
        let encrypted = symm::encrypt(cipher, aes, None, &data)?;

        let size = (encrypted.len() as u32).to_be_bytes();

        writer.write(&size).await?;
        writer.write_all(&encrypted).await?;

        Ok(())
    }

    pub async fn write<T: AsyncWriteExt + Unpin>(&self, writer: &mut T) -> Result<(), anyhow::Error> {
        let data = rmp_serde::to_vec_named(&self)?;
        let size = (data.len() as u32).to_be_bytes();

        writer.write(&size).await?;
        writer.write_all(&data).await?;

        Ok(())
    }

    // pub async fn read_aes<T: AsyncReadExt + Unpin>(reader: &mut T, aes: &[u8; 32], iv: &[u8; 32]) -> Result<Self, Box<dyn Error + 'static>> {
    pub async fn read_aes<T: AsyncReadExt + Unpin>(reader: &mut T, aes: &[u8]) -> Result<Self, anyhow::Error> {
        let mut size_bytes = [0; 4];
        reader.read_exact(&mut size_bytes).await?;
        let size = u32::from_be_bytes(size_bytes) as usize;
        let mut buffer = vec![0u8; size];
        reader.read_exact(&mut buffer).await?;

        let cipher = symm::Cipher::aes_256_ctr();
        let data = symm::decrypt(cipher, aes, None, &buffer)?;

        Ok(rmp_serde::from_slice(&data)?)
    }

    pub async fn read<T: AsyncReadExt + Unpin>(reader: &mut T) -> Result<Self, Box<dyn Error>> {
        let mut size_bytes = [0; 4];
        reader.read_exact(&mut size_bytes).await?;
        let size = u32::from_be_bytes(size_bytes) as usize;
        let mut buffer = vec![0u8; size];
        reader.read_exact(&mut buffer).await?;

        Ok(rmp_serde::from_slice(&buffer)?)
    }
}

impl<M> From<M> for Envelope<M> {
    fn from(m: M) -> Self {
        Envelope::Message(m)
    }
}