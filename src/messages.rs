
use std::fmt::Debug;

use serde::{Serialize, Deserialize};

use crate::{
    blockchain::{Data, Block},
    highlander::Game
};


#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum Messages<T> {
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
    UserData(T),
}
