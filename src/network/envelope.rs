use serde::{Serialize, Deserialize, de::DeserializeOwned};

use crate::key::PubKey;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag="kind")]
pub enum Envelope<T> {
    Greeting {
        #[serde(with="serde_bytes")]
        id: PubKey,
        #[serde(with="serde_bytes")]
        shared: PubKey,
        thin: bool
    },
    AllKnown { 
        all_known: Vec<serde_bytes::ByteBuf>
    },
    Announce {
        #[serde(with="serde_bytes")]
        id: PubKey
    },
    Remove {
        #[serde(with="serde_bytes")]
        id: PubKey
    },
    Message(T),
}

impl<T> Envelope<T> 
  where T: Serialize + DeserializeOwned {
    pub fn to_bytes(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec_named(self)
    }

    pub fn from_bytes(v: &Vec<u8>) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(v)
    }
}

impl<M> From<M> for Envelope<M> {
    fn from(m: M) -> Self {
        Envelope::Message(m)
    }
}