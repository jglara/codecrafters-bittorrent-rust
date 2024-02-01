use anyhow::Context;
use bittorrent_starter_rust::parser::decode_bencoded_value;
use bittorrent_starter_rust::peer::Peer;
use bittorrent_starter_rust::torrent::TorrentFile;
use bittorrent_starter_rust::tracker::Tracker;
use clap::Parser;
use clap::Subcommand;

use clap;
use serde_bencode;
use std::fs;

use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Decode { value: String },
    Info { path: PathBuf },
    Peers { path: PathBuf },
    Handshake { path: PathBuf, peer: SocketAddr },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Decode { value } => {
            let decoded_value = decode_bencoded_value(&value);
            println!("{decoded_value}");
        }
        Command::Info { path } => {
            let content = fs::read(path).context("Reading torrent file")?;
            let torrent =
                serde_bencode::from_bytes::<TorrentFile>(&content).context("parse torrent file")?;

            println!("Tracker URL: {}", torrent.announce);
            println!("Length: {}", torrent.info.length);
            println!("Info Hash: {}", hex::encode(torrent.info.hash()?));
            println!("Piece Length: {}", torrent.info.piece_length);
            println!("Piece Hashes:");
            torrent
                .info
                .piece_hashes()?
                .iter()
                .for_each(|h| println!("{}", hex::encode(h)));
        }
        Command::Peers { path } => {
            let content = fs::read(path).context("Reading torrent file")?;
            let torrent =
                serde_bencode::from_bytes::<TorrentFile>(&content).context("parse torrent file")?;

            let tracker = Tracker::new();
            let peers: Vec<_> = tracker.req_peers(&torrent).await?;

            peers.iter().for_each(|p| println!("{p:?}"));
        }

        Command::Handshake { path, peer } => {
            eprintln!("{path:?} {peer:?}");
            let content = fs::read(path).context("Reading torrent file")?;
            let torrent =
                serde_bencode::from_bytes::<TorrentFile>(&content).context("parse torrent file")?;

            let peer = Peer::connect(peer, &torrent).await?;

            println!("Peer ID: {}", hex::encode(peer.remote_id));
        }
    }

    Ok(())
}
