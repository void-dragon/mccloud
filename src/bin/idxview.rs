use std::collections::HashMap;

use clap::Parser;

#[derive(Parser)]
struct Args {
    #[clap(long, short)]
    file: String,
}

fn main() {
    let args = Args::parse();

    let data = std::fs::read(args.file).unwrap();
    let index: HashMap<Vec<u8>, (u64, u64)> = bincode::deserialize(&data).unwrap();
    let mut sorted = Vec::new();

    for (k, v) in index.iter() {
        sorted.push((k, v.0, v.1));
    }

    sorted.sort_by_cached_key(|x| x.1);

    for k in sorted {
        println!("  {} -> {} : {}", hex::encode(k.0), k.1, k.2);
    }
}