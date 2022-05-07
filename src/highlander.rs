use std::collections::HashMap;

use openssl::{rand::rand_bytes, sha::Sha512};
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

///
/// A game of a single node.
/// 
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Game {
    /// The public key of the playing node.
    author: PubKey,
    /// The signature over the player rounds.
    sign: Vec<u8>,
    /// The choices of the node for the game rounds.
    rounds: Vec<u8>,
}

///
/// The final game result to be shared.
/// 
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameResult {
    /// The game tree of the matches.
    pub tree: Vec<Option<PubKey>>,
    /// The single nodes and their choices.
    pub roster: HashMap<PubKey, Vec<u8>>,
    /// The winner and author of this game result.
    pub winner: PubKey,
    /// The signature by the winner.
    pub sign: Vec<u8>,
}

fn game_result_hash(tree: &Vec<Option<PubKey>>, roster: &HashMap<PubKey, Vec<u8>>, winner: &PubKey) -> [u8; 64] {
    let mut sha = Sha512::new();

    for id in tree {
        if let Some(ref id) = id {
            sha.update(id);
        }
    }

    for (id ,v) in roster {
        sha.update(id);
        sha.update(v);
    }

    sha.update(winner);

    sha.finish()
}

impl GameResult {
    fn build(tree: Vec<Option<PubKey>>, roster: &HashMap<PubKey, Option<Vec<u8>>>, key: &Key) -> Self {
        let roster: HashMap<PubKey, Vec<u8>> = roster
            .iter()
            .map(|k| (k.0.clone(), k.1.as_ref().unwrap().clone()))
            .collect();
        let winner = tree.last().unwrap().clone().unwrap();

        let sign = if key.public_key == winner {
            let hash = game_result_hash(&tree, &roster, &winner);
            key.sign(&hash).unwrap()
        }
        else {
            Vec::new()
        };

        Self {
            tree,
            roster,
            winner,
            sign,
        }
    }
}

///
/// An abstraction of the Highlander algorithm.
/// 
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

    pub fn add_game(&mut self, game: Game) -> bool {
        let count = self.roster.len();
        let count = count + count % 2;
        let count = (count as f64).log2() as usize;

        if game.rounds.len() == count {
            if self.roster.contains_key(&game.author) {
                self.roster.insert(game.author, Some(game.rounds));
                true
            }
            else {
                log::error!("game author is not part of the game");
                false
            }
        }
        else {
            log::error!("round length for game does not match");
            false
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

        let winner = tree.last().unwrap().clone().unwrap();
        log::info!("winner {}", hex::encode(&winner));

        let result = GameResult::build(tree, &self.roster, key);

        self.roster.clear();

        result
    }
}