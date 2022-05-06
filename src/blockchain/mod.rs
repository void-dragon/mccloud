pub mod block;
pub mod data;

use crate::{
    highlander::GameResult,
    key::Key
};

pub use self::{
    block::Block,
    data::Data,
};

pub struct Blockchain {
    bucket: Vec<Data>,
    highest_hash: Vec<u8>,
}

impl Blockchain {
    pub fn new() -> Self {
        Self {
            bucket: Vec::new(),
            highest_hash: Vec::new(),
        }
    }

    pub fn add_to_cache(&mut self, data: Data) {
        self.bucket.push(data);
    }

    pub fn generate_new_block(&mut self, game: GameResult, key: &Key) -> Block {
        let block = Block::build(
            &self.highest_hash,
            game,
            key,
            self.bucket.drain(..).collect()
        );

        self.highest_hash = block.hash.clone();

        block
    }

    pub fn add_new_block(&mut self, block: Block) {
        block.validate();
    }
}