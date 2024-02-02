use anyhow::{anyhow, Context};
use bittorrent_starter_rust::parser::decode_bencoded_value;
use bittorrent_starter_rust::peer::Peer;
use bittorrent_starter_rust::torrent::TorrentFile;
use bittorrent_starter_rust::tracker::Tracker;
use clap::Parser;
use clap::Subcommand;

use clap;
use serde_bencode;
use std::fs;

use std::io::Write;
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
    Decode {
        value: String,
    },
    Info {
        path: PathBuf,
    },
    Peers {
        path: PathBuf,
    },
    Handshake {
        path: PathBuf,
        peer: SocketAddr,
    },
    #[command(rename_all = "snake_case")]
    DownloadPiece {
        #[arg(short)]
        output: PathBuf,
        path: PathBuf,
        piece_id: usize,
    },
    Download {
        #[arg(short)]
        output: PathBuf,
        path: PathBuf,
    },
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
        Command::DownloadPiece {
            output,
            path,
            piece_id,
        } => {
            let content = fs::read(path).context("Reading torrent file")?;
            let torrent =
                serde_bencode::from_bytes::<TorrentFile>(&content).context("parse torrent file")?;

            let tracker = Tracker::new();
            let peers: Vec<_> = tracker.req_peers(&torrent).await?;

            let mut peer =
                Peer::connect(*peers.first().ok_or_else(|| anyhow!("No peers"))?, &torrent).await?;

            peer.recv_bitfield().await?;
            let piece_length = if piece_id == torrent.info.piece_hashes()?.len() - 1 {
                torrent.info.length % torrent.info.piece_length
            } else {
                torrent.info.piece_length
            };

            let bytes = peer
                .download_piece(
                    piece_id,
                    piece_length,
                    torrent.info.piece_hashes()?[piece_id],
                )
                .await?;

            fs::write(&output, bytes)?;
            println!("Piece {piece_id} downloaded to {}", output.display());
        }
        Command::Download { output, path } => {
            let content = fs::read(&path).context("Reading torrent file")?;
            let torrent =
                serde_bencode::from_bytes::<TorrentFile>(&content).context("parse torrent file")?;

            let tracker = Tracker::new();
            let peers: Vec<_> = tracker.req_peers(&torrent).await?;

            let mut peer =
                Peer::connect(*peers.first().ok_or_else(|| anyhow!("No peers"))?, &torrent).await?;

            peer.recv_bitfield().await?;

            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&output)?;

            for (piece_id, piece_length, piece_hash) in torrent
                .info
                .piece_hashes()?
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    (
                        i,
                        if i == torrent.info.piece_hashes().unwrap().len() - 1 {
                            torrent.info.length % torrent.info.piece_length
                        } else {
                            torrent.info.piece_length
                        },
                        h,
                    )
                })
            {
                let bytes = peer
                    .download_piece(piece_id, piece_length, piece_hash)
                    .await?;

                file.write(&bytes[..])?;

                eprintln!("Piece {piece_id} downloaded to {}", output.display());
            }

            file.flush()?;

            println!("Downloaded {} to {}.", path.display(), output.display());
        }
    }

    Ok(())
}
