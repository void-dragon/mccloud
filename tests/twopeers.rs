
use std::{time::Duration, path::Path};

use mccloud::{
    config::{Config, ClientConfig},
    network::{peer::Peer, handler::daemon::DaemonHandler},
};

mod testclient;
use testclient::TestHandler;

fn save_remove(filename: &str) {
    let path = Path::new(filename);

    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[tokio::test]
async fn two_peers() {
    let env = env_logger::Env::default().default_filter_or("debug");
    env_logger::init_from_env(env);
    
    save_remove("data/test00");
    save_remove("data/test01");

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
            ClientConfig {host: "127.0.0.1".into(), port: 39093, reconnect: false}
        ],
    };
    let peer01 = Peer::<DaemonHandler>::new(config);
    let p01 = peer01.clone();

    let config = Config {
        host: "127.0.0.1".into(),
        port: 39095,
        thin: true,
        folder: "data/client00".into(),
        clients: vec![
            ClientConfig {host: "127.0.0.1".into(), port: 39093, reconnect: false}
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

    tokio::time::sleep(Duration::from_millis(500)).await;

    let all_kn_cnt00 = peer00.all_known.lock().await.len();
    let all_kn_cnt01 = peer01.all_known.lock().await.len();

    assert_eq!(all_kn_cnt00, 2);
    assert_eq!(all_kn_cnt01, 2);

    peer01.shutdown();
    peer00.shutdown();
}