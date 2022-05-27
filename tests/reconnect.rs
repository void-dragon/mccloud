
use std::time::Duration;

use mccloud::{
    config::{Config, ClientConfig},
    network::{peer::Peer, handler::daemon::DaemonHandler},
};

#[tokio::test]
async fn two_peers() {
    let env = env_logger::Env::default().default_filter_or("debug");
    env_logger::init_from_env(env);

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
            ClientConfig {host: "127.0.0.1".into(), port: 39093, reconnect: true}
        ],
    };
    let peer01 = Peer::<DaemonHandler>::new(config);
    let p01 = peer01.clone();

    tokio::spawn(async move {
        p00.listen().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(250)).await;
    tokio::spawn(async move {
        p01.listen().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(250)).await;
    peer00.shutdown();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let all_kn_cnt00 = peer00.all_known.lock().await.len();
    let all_kn_cnt01 = peer01.all_known.lock().await.len();

    assert_eq!(all_kn_cnt00, 1);
    assert_eq!(all_kn_cnt01, 1);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let p00 = peer00.clone();
    tokio::spawn(async move {
        p00.listen().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(2500)).await;

    let all_kn_cnt00 = peer00.all_known.lock().await.len();
    let all_kn_cnt01 = peer01.all_known.lock().await.len();

    assert_eq!(all_kn_cnt00, 2);
    assert_eq!(all_kn_cnt01, 2);

    peer00.shutdown();
    peer01.shutdown();
}