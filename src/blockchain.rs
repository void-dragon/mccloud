use openssl::sha::Sha256;
use serde::{Serialize, Deserialize};

use crate::{key::{Key, PubKey}, highlander::GameResult};


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Data {
    pub data: Vec<u8>,
    pub author: PubKey,
    pub sign: Vec<u8>,
}

impl Data {
    pub fn build(key: &Key, data: Vec<u8>) -> Self {
        let sign = key.sign(&data).unwrap();
        Self {
            author: key.public_key.clone(),
            sign,
            data,
        }
    }

    pub fn validate(&self) -> bool {
        match Key::validate(&self.data, &self.author, &self.sign) {
            Ok(valid) => valid,
            Err(e) => {
                log::error!("unvalid data chunk: {}", e);
                false
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Block {
    game: GameResult,
    data: Vec<Data>,
    author: PubKey,
    sign: Vec<u8>,
}

fn block_hash(data: &Vec<Data>, game: &GameResult) -> [u8; 32] {
    let mut sha = Sha256::new();

    for d in data {
        sha.update(&d.data);
        sha.update(&d.author);
        sha.update(&d.sign);
    }

    for id in &game.tree {
        if let Some(ref id) = id {
            sha.update(id);
        }
    }

    for (id ,v) in &game.roster {
        sha.update(id);
        sha.update(v);
    }

    sha.update(&game.winner);

    sha.finish()
}

impl Block {
    pub fn validate(&self) -> bool {
        for d in &self.data {
            if !d.validate() {
                return false
            }
        }

        let hash = block_hash(&self.data, &self.game);

        match Key::validate(&hash, &self.author, &self.sign) {
            Ok(valid) => valid,
            Err(e) => {
                log::error!("unvalid block: {}", e);
                false
            }
        }
    }
}

pub struct Blockchain {
    bucket: Vec<Data>,
}

impl Blockchain {
    pub fn new() -> Self {
        Self {
            bucket: Vec::new(),
        }
    }

    pub fn add_to_cache(&mut self, data: Data) {
        self.bucket.push(data);
    }

    pub fn generate_new_block(&mut self, game: GameResult, key: &Key) -> Block {
        let hash = block_hash(&self.bucket, &game);
        let sign = key.sign(&hash).unwrap();

        let block = Block {
            game,
            data: self.bucket.drain(..).collect(),
            author: key.public_key.clone(),
            sign,
        };

        block
    }

    pub fn add_new_block(&mut self, block: Block) {
        block.validate();
    }
}