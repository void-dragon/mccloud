use serde::{Deserialize, Serialize};

use crate::key::{PubKey, Key};



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