pub mod block;
pub mod data;

use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    collections::HashMap,
    io::{Seek, Write}
};

use crate::{
    highlander::GameResult,
    key::Key
};

pub use self::{
    block::Block,
    data::Data,
};

pub struct Blockchain {
    folder: PathBuf,
    bucket: Vec<Data>,
    highest_hash: Vec<u8>,
    index: HashMap<Vec<u8>, (u64, u64)>,
}

impl Blockchain {
    pub fn new(folder: &str) -> Self {
        let folder = Path::new(folder).to_path_buf();

        if !folder.exists() {
            std::fs::create_dir_all(&folder).unwrap();
        }

        let idxname = folder.join("bc.idx");

        let index: HashMap<Vec<u8>, (u64, u64)> = if idxname.exists() {
            let file = File::open(idxname).unwrap();
            rmp_serde::from_read(file).unwrap()
        }
        else {
            HashMap::new()
        };
        
        let mut hh = Vec::new();
        let mut hidx = 0;
        for (h, p) in index.iter() {
            if p.0 >= hidx {
                hidx = p.0;
                hh = h.clone();
            }
        }
        
        log::debug!("highest hash {}", hex::encode(&hh));

        Self {
            folder,
            bucket: Vec::new(),
            highest_hash: hh,
            index,
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

        self.save_block(&block);

        block
    }

    pub fn add_new_block(&mut self, block: Block) {
        if block.validate() {
            if self.highest_hash == block.parent {
                self.save_block(&block);
            }
            else {
                log::error!(
                    "new block has not current highest block as parent:\nnode:    {}\nparent:  {}\nhighest: {}",
                    hex::encode(&block.hash),
                    hex::encode(&block.parent),
                    hex::encode(&self.highest_hash)
                );
            }
        }
        else {
            log::error!("invalid block {}", hex::encode(&block.hash));
        }
    }

    fn save_block(&mut self, block: &Block) {
        log::info!("save block {}", hex::encode(&block.hash));

        let filename = self.folder.join("bc.db");
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(filename).unwrap();
        let pos = file.seek(std::io::SeekFrom::End(0)).unwrap();       
        let data = rmp_serde::to_vec_named(&block).unwrap();
        let end = data.len() as u64;
        file.write_all(&data).unwrap();

        self.index.insert(block.hash.clone(), (pos, end));
    }

    pub fn save_index(&self) {
        let filename = self.folder.join("bc.idx");
        let mut file = File::create(filename).unwrap();
        let data = rmp_serde::to_vec_named(&self.index).unwrap();
        file.write_all(&data).unwrap();
    }
}