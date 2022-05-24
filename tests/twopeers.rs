
use mccloud::{
    config::{Config, ClientConfig},
    network::{peer::Peer, handler::daemon::DaemonHandler},
};

mod testclient;
use testclient::TestHandler;

#[tokio::test]
async fn two_peers() {
    let config = Config {
        host: "127.0.0.1".into(),
        port: 39093,
        thin: false,
        folder: "data/test00".into(),
        clients: Vec::new(),
    };
    let peer00 = Peer::<DaemonHandler>::new(config);
    let p00 = peer00.clone();

    let config = Config {
        host: "127.0.0.1".into(),
        port: 39094,
        thin: false,
        folder: "data/test01".into(),
        clients: vec![
            ClientConfig {host: "127.0.0.1".into(), port: 39093}
        ],
    };
    let peer01 = Peer::<DaemonHandler>::new(config);
    let p01 = peer01.clone();

    let config = Config {
        host: "127.0.0.1".into(),
        port: 39095,
        thin: false,
        folder: "data/client00".into(),
        clients: vec![
            ClientConfig {host: "127.0.0.1".into(), port: 39093}
        ],
    };
    let client = Peer::<TestHandler>::new(config);
    let c00 = client.clone();

    tokio::spawn(async move {
        p00.listen().await.unwrap();
    });
    tokio::spawn(async move {
        p01.listen().await.unwrap();
    });
    tokio::spawn(async move {
        c00.listen().await.unwrap();
    });

    let all_kn_cnt00 = peer00.all_known.lock().await.len();
    let all_kn_cnt01 = peer01.all_known.lock().await.len();

    assert!(all_kn_cnt00 == all_kn_cnt01);

    peer01.shutdown();
    peer00.shutdown();
}