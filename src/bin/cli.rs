use std::net::TcpStream;

use clap::Parser;
use cluster_rs::{messages::Messages, blockchain::Data, key::Key};

/// cli connection to the cluster
#[derive(Parser)]
#[clap(author, version, about)]
struct Args {
    /// host name or ip
    #[clap(long, short)]
    host: String,
    /// the rpc port
    #[clap(long, short)]
    port: u16,
}


fn main() {
    let args = Args::parse();

    let mut stream = TcpStream::connect((args.host, args.port)).unwrap();

    let _greeting = Messages::read(&mut stream).unwrap();

    let bytes: Vec<u8> = vec![0, 1, 2];
    let key = Key::load("etc/user.key").unwrap();
    let data = Data::build(&key, bytes);
    let share = Messages::Share { data };
    share.write(&mut stream).unwrap();
}