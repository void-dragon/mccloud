use std::{pin::Pin, future::Future};

use crate::{
    network::peer::Peer,
    config::Config
};

use super::{client::ClientPtr, message::Message};

pub mod daemon;

pub trait Handler: Send + Sync + Clone {
    fn new(config: &Config) -> Self;

    fn init<'a>(&'a self, peer: Peer<Self>, client: ClientPtr) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a;

    fn shutdown<'a>(&'a self, peer: Peer<Self>) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a;

    fn handle<'a>(&'a self, peer: Peer<Self>, client: ClientPtr, msg: Message) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a;
}