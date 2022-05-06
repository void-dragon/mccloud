use std::error::Error;

use openssl::{
    pkey::PKey,
    encrypt::{Encrypter, Decrypter},
    // ec::EcKey,
    symm
};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use tokio::io::{AsyncWriteExt, AsyncReadExt};

use crate::key::{PubKey, Key};

#[derive(Serialize, Deserialize, Debug)]
pub enum Envelope<T> {
    Greeting {id: PubKey, thin: bool },
    AesKey {aes: [u8; 32], iv: [u8; 16]},
    AllKnown { all_known: Vec<PubKey>},
    Announce {id: PubKey},
    Remove {id: PubKey},
    Message(T),
}

impl<M> Envelope<M> 
where
    M: Serialize + DeserializeOwned
{
    pub async fn write_aes<T: AsyncWriteExt + Unpin>(&self, writer: &mut T, aes: &[u8; 32], iv: &[u8; 16]) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(&self)?;
        let cipher = symm::Cipher::aes_256_cbc();
        let encrypted = symm::encrypt(cipher, aes, Some(iv), &data)?;

        let size = (encrypted.len() as u32).to_be_bytes();

        writer.write(&size).await?;
        writer.write_all(&encrypted).await?;

        Ok(())
    }

    pub async fn write_ec<T: AsyncWriteExt + Unpin>(&self, writer: &mut T, key: &Vec<u8>) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(&self)?;
        let pkey = PKey::public_key_from_der(key)?;
        let encrypter = Encrypter::new(&pkey).unwrap();
        
        let buffer_len = encrypter.encrypt_len(&data)?;
        let mut buffer = vec![0; buffer_len];
        let enc_len = encrypter.encrypt(&data, &mut buffer)?;
        buffer.truncate(enc_len);

        let size = (buffer.len() as u32).to_be_bytes();

        writer.write(&size).await?;
        writer.write_all(&buffer).await?;

        Ok(())
    }

    pub async fn write<T: AsyncWriteExt + Unpin>(&self, writer: &mut T) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(&self)?;
        let size = (data.len() as u32).to_be_bytes();

        writer.write(&size).await?;
        writer.write_all(&data).await?;

        Ok(())
    }

    // pub async fn read_aes<T: AsyncReadExt + Unpin>(reader: &mut T, aes: &[u8; 32], iv: &[u8; 32]) -> Result<Self, Box<dyn Error + 'static>> {
    pub async fn read_aes<T: AsyncReadExt + Unpin>(reader: &mut T, aes: &[u8; 32], iv: &[u8; 16]) -> Result<Self, anyhow::Error> {
        let mut size_bytes = [0; 4];
        reader.read_exact(&mut size_bytes).await?;
        let size = u32::from_be_bytes(size_bytes) as usize;
        let mut buffer = vec![0u8; size];
        reader.read_exact(&mut buffer).await?;

        let cipher = symm::Cipher::aes_256_cbc();
        let data = symm::decrypt(cipher, aes, Some(iv), &buffer)?;

        Ok(bincode::deserialize(&data)?)
    }

    pub async fn read_ec<T: AsyncReadExt + Unpin>(reader: &mut T, key: &Key) -> Result<Self, Box<dyn Error>> {
        let mut size_bytes = [0; 4];
        reader.read_exact(&mut size_bytes).await?;
        let size = u32::from_be_bytes(size_bytes) as usize;
        let mut buffer = vec![0u8; size];
        reader.read_exact(&mut buffer).await?;

        let decrypter = Decrypter::new(&key.private_key)?;

        let dec_len = decrypter.decrypt_len(&buffer)?;
        let mut data = vec![0; dec_len];
        let dec_len = decrypter.decrypt(&buffer, &mut data)?;
        data.truncate(dec_len);

        Ok(bincode::deserialize(&data)?)
    }

    pub async fn read<T: AsyncReadExt + Unpin>(reader: &mut T) -> Result<Self, Box<dyn Error>> {
        let mut size_bytes = [0; 4];
        reader.read_exact(&mut size_bytes).await?;
        let size = u32::from_be_bytes(size_bytes) as usize;
        let mut buffer = vec![0u8; size];
        reader.read_exact(&mut buffer).await?;

        Ok(bincode::deserialize(&buffer)?)
    }
}

impl<M> From<M> for Envelope<M> {
    fn from(m: M) -> Self {
        Envelope::Message(m)
    }
}