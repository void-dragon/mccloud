
use serde::{Serialize, Deserialize};

use crate::{
    blockchain::{Data, Block},
    highlander::Game
};


#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Messages {
    Share {data: Data},
    Play {game: Game},
    ShareBlock { block: Block },
}
