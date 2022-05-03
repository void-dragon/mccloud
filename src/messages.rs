use std::{net::TcpStream, error::Error, io::{Write, Read}};

use serde::{Serialize, Deserialize};

use crate::{blockchain::{Data, Block}, highlander::Game, key::PubKey};


#[derive(Serialize, Deserialize, Debug)]
pub enum Messages {
    Greeting {id: PubKey, all_known: Vec<PubKey>},
    Announce {id: PubKey},
    Remove {id: PubKey},
    Share {data: Data},
    Play {game: Game},
    ShareBlock { block: Block },
}

impl Messages {
    pub fn write(&self, writer: &mut TcpStream) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(&self)?;
        let size = (data.len() as u32).to_be_bytes();

        writer.write(&size)?;
        writer.write_all(&data)?;

        Ok(())
    }

    pub fn read(reader: &mut TcpStream) -> Result<Messages, Box<dyn Error>> {
        let mut size_bytes = [0; 4];
        reader.read_exact(&mut size_bytes)?;
        let size = u32::from_be_bytes(size_bytes) as usize;
        let mut buffer = vec![0u8; size];
        reader.read_exact(&mut buffer)?;

        let msg: Messages = bincode::deserialize(&buffer)?;

        Ok(msg)
    }
}