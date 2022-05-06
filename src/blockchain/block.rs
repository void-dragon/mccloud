use openssl::sha::Sha512;
use serde::{Serialize, Deserialize};

use crate::{highlander::GameResult, key::{PubKey, Key}};

use super::data::Data;


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Block {
    pub parent: Vec<u8>,
    pub game: GameResult,
    pub data: Vec<Data>,
    /// The public key of the node which created the block.
    pub author: PubKey,
    /// The hash of the block data.
    pub hash: Vec<u8>,
    /// The sign for the [GameResult] and [Data] blocks.
    pub sign: Vec<u8>,
}

pub fn block_hash(parent: &Vec<u8>, author: &Vec<u8>, data: &Vec<Data>, game: &GameResult) -> [u8; 64] {
    let mut sha = Sha512::new();

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

    sha.update(parent);
    sha.update(author);

    sha.finish()
}

impl Block {
    pub fn build(parent: &Vec<u8>, game: GameResult, key: &Key, data: Vec<Data>) -> Block {
        let author = key.public_key.clone();
        let hash = block_hash(parent, &author, &data, &game);
        let sign = key.sign(&hash).unwrap();

        Block {
            parent: parent.clone(),
            game,
            data,
            author,
            hash: hash.to_vec(),
            sign,
        }
    }

    pub fn validate(&self) -> bool {
        for d in &self.data {
            if !d.validate() {
                return false
            }
        }

        let hash = block_hash(&self.parent, &self.author, &self.data, &self.game);

        if self.hash == hash {
            match Key::validate(&hash, &self.author, &self.sign) {
                Ok(valid) => valid,
                Err(e) => {
                    log::error!("unvalid block: {}", e);
                    false
                }
            }
        }
        else {
            false
        }
    }
}