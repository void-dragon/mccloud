use serde::{Serialize, Deserialize};

use crate::{
    key::PubKey,
    blockchain::{Data, Block},
    highlander::Game
};

///
/// The network layer message
/// 
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag="kind")]
pub enum Message {
    Greeting {
        #[serde(with="serde_bytes")]
        id: PubKey,
        #[serde(with="serde_bytes")]
        shared: PubKey,
        thin: bool
    },
    AllKnown { 
        all_known: Vec<serde_bytes::ByteBuf>
    },
    Announce {
        #[serde(with="serde_bytes")]
        id: PubKey
    },
    Remove {
        #[serde(with="serde_bytes")]
        id: PubKey
    },
    HighestBlock{
        #[serde(with="serde_bytes")]
        hash: Vec<u8>,
        count: usize
    },
    RequestBlocks{
        #[serde(with="serde_bytes")]
        from: Vec<u8>,
        #[serde(with="serde_bytes")]
        to: Vec<u8>
    },
    Blocks {blocks: Vec<Block>},
    Share {data: Data},
    Play {game: Game},
    AddBlock { block: Block },
}

impl Message {
    pub fn to_bytes(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec_named(self)
    }

    pub fn from_bytes(v: &Vec<u8>) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(v)
    }
}