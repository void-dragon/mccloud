use std::collections::HashMap;

use openssl::{rand::rand_bytes, sha::Sha256};
use serde::{Serialize, Deserialize};

use crate::key::{Key, PubKey};

const ROCK: u8 = 0;
// const PAPER: u8 = 1;
const SCISSOR: u8 = 2;


fn winner(p0: PubKey, v0: u8, p1: PubKey, v1: u8) -> PubKey {
    if v0 > v1 {
        if v0 == SCISSOR && v1 == ROCK {
            p1
        }
        else {
            p0
        }
    }
    else {
        if v0 == ROCK && v1 == SCISSOR {
            p0
        }
        else {
            p1
        }
    }
}

struct IntIter {
    pos: usize,
    step: usize,
    end: usize,
}

impl Iterator for IntIter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.end {
            let val = self.pos;
            self.pos += self.step;
            Some(val)
        }
        else {
            None
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Game {
    author: PubKey,
    sign: Vec<u8>,
    rounds: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameResult {
    pub tree: Vec<Option<PubKey>>,
    pub roster: HashMap<PubKey, Vec<u8>>,
    pub winner: PubKey,
    pub sign: Vec<u8>,
}

impl GameResult {
    fn build(tree: Vec<Option<PubKey>>, roster: &HashMap<PubKey, Option<Vec<u8>>>, key: &Key) -> Self {
        let roster: HashMap<PubKey, Vec<u8>> = roster
            .iter()
            .map(|k| (k.0.clone(), k.1.as_ref().unwrap().clone()))
            .collect();
        // for v in &tree {
        //     if let Some(ref v) = v {
        //         println!("{}", hex::encode(v));
        //     }
        //     else {
        //         println!("None");
        //     }
        // }
        let winner = tree.last().unwrap().clone().unwrap();
        let mut sha = Sha256::new();

        for id in &tree {
            if let Some(ref id) = id {
                sha.update(id);
            }
        }

        for (id ,v) in &roster {
            sha.update(id);
            sha.update(v);
        }

        sha.update(&winner);

        let hash = sha.finish();
        let sign = key.sign(&hash).unwrap();

        Self {
            tree,
            roster,
            winner,
            sign,
        }
    }
}

pub struct Highlander {
    roster: HashMap<PubKey, Option<Vec<u8>>>,
}

impl Highlander {
    pub fn new() -> Self {
        Self {
            roster: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.roster.clear();
    }

    pub fn populate_roster<'a, T: Iterator<Item=&'a PubKey>>(&mut self, iter: T) {
        for id in iter {
            self.roster.insert(id.clone(), None);
        }
    }

    pub fn add_game(&mut self, game: Game) {
        if self.roster.contains_key(&game.author) {
            self.roster.insert(game.author, Some(game.rounds));
        }
    }

    pub fn create_game(&self, key: &Key) -> Game {
        let count = self.roster.len();
        let count = count + count % 2;
        let count = (count as f64).log2() as usize;
        let mut buf = vec![0u8; count];

        rand_bytes(&mut buf).unwrap();

        for i in 0..count {
            buf[i] %= 3;
        }

        let sign = key.sign(&buf).unwrap();

        Game { 
            author: key.public_key.clone(), 
            sign,
            rounds: buf
        }
    }

    pub fn is_filled(&self) -> bool {
        for val in self.roster.values() {
            if val.is_none() {
                return false
            }
        }

        true
    }

    pub fn evaluate(&mut self, key: &Key) -> GameResult {
        let count = self.roster.len();
        let count = (count + count % 2) * 2 - 1;
        let mut tree: Vec<Option<PubKey>> = vec![None; count];

        let mut ids: Vec<PubKey> = self.roster.keys().cloned().collect();
        ids.sort();

        for (i, id) in ids.into_iter().enumerate() {
            tree[i] = Some(id);
        }
        
        let mut lvl = 0;
        let mut offset = 0;
        let count = self.roster.len();
        let mut count = count + count % 2;

        while count > 1 {
            let rng = IntIter{ pos: 0, end: count, step: 2 };
            for i in rng {
                let p0 = tree[offset + i].clone().unwrap();
                let v0 = self.roster[&p0].as_ref().unwrap()[lvl];

                let w = if let Some(ref p1) = tree[offset + i + 1] {
                    let v1 = self.roster[p1].as_ref().unwrap()[lvl];

                    winner(p0, v0, p1.clone(), v1)
                }
                else {
                    p0
                };
                    
                tree[i / 2 + count + offset] = Some(w);
            }
            
            offset += count;
            count = count / 2;
            lvl += 1;
        }

        let result = GameResult::build(tree, &self.roster, key);

        self.roster.clear();

        result
    }
}