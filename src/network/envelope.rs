use serde::{Serialize, Deserialize};

use crate::key::PubKey;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag="kind")]
pub enum Envelope<T> {
    Greeting {
        #[serde(with="serde_bytes")]
        id: PubKey,
        thin: bool
    },
    AllKnown { all_known: Vec<PubKey>},
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

impl<M> From<M> for Envelope<M> {
    fn from(m: M) -> Self {
        Envelope::Message(m)
    }
}