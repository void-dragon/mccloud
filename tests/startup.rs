use mccloud::{
    config::Config,
    network::{peer::Peer, handler::daemon::DaemonHandler},
};
use std::time::Duration;
use tokio::time::sleep;


#[tokio::test]
async fn simple_start() {
    let config = Config {
        host: "127.0.0.1".into(),
        port: 39093,
        thin: false,
        folder: "data/test".into(),
        clients: Vec::new(),
    };
    let peer = Peer::<DaemonHandler>::new(config);
    let pc = peer.clone();

    tokio::spawn(async move {
        sleep(Duration::from_millis(500)).await;
        pc.shutdown();
    });

    let result = peer.listen().await;
    assert!(result.is_ok());
}