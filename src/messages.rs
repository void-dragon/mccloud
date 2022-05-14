
use std::fmt::Debug;

use serde::{Serialize, Deserialize};

use crate::{
    blockchain::{Data, Block},
    highlander::Game
};


#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Messages<T> {
    HighestBlock{hash: Vec<u8>, count: usize},
    RequestBlocks{from: Vec<u8>, to: Vec<u8>},
    Blocks {blocks: Vec<Block>},
    Share {data: Data},
    Play {game: Game},
    AddBlock { block: Block },
    UserData(T),
}
