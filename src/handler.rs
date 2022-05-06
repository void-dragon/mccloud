use std::{pin::Pin, future::Future};

use crate::network::peer::{Peer, ClientPtr};


pub trait Handler: Send + Sync {
    type Msg;

    fn new() -> Self;

    fn init<'a>(&'a self, peer: Peer<Self>, client: ClientPtr) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a;

    fn handle<'a>(&'a self, peer: Peer<Self>, client: ClientPtr, msg: Self::Msg) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: Sync + 'a;
}