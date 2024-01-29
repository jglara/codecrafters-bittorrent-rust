use anyhow::{anyhow, Context};
use bittorrent_starter_rust::parser::decode_bencoded_value;
use bittorrent_starter_rust::peer::Handshake;
use bittorrent_starter_rust::peer::HANDSHAKE_LEN;

use clap::Parser;
use clap::Subcommand;
use reqwest::Client;

use serde::{Deserialize, Serialize};

use sha1::{Digest, Sha1};

use clap;
use serde_bencode;
use std::fs;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use tokio::net::TcpStream;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TorrentInfo {
    name: String,

    #[serde(rename = "piece length")]
    piece_length: usize,

    #[serde(with = "serde_bytes")]
    pieces: Vec<u8>,

    length: usize,
}

impl TorrentInfo {
    fn piece_hashes(&self) -> anyhow::Result<Vec<&[u8; 20]>> {
        if self.pieces.len() % 20 == 0 {
            self.pieces
                .chunks_exact(20)
                .map(|c| <&[u8; 20]>::try_from(c))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow!(e))
                .context("Extracting hashes")
        } else {
            Err(anyhow!("Invalid hashes length {}", self.pieces.len()))
        }
    }

    fn hash(&self) -> anyhow::Result<[u8; 20]> {
        let info = serde_bencode::to_bytes(&self)?;
        let mut hasher = Sha1::new();
        hasher.update(&info);
        let hashed_info = hasher.finalize();

        hashed_info[..].try_into().map_err(|e| anyhow!("{}", e))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TorrentFile {
    announce: String,
    info: TorrentInfo,
}

#[derive(Debug, Clone, Serialize)]
struct TrackerRequest {
    //#[serde(serialize_with="hash_encode")]
    //info_hash: [u8; 20],
    peer_id: String,
    port: u16,
    uploaded: usize,
    downloaded: usize,
    left: usize,
    compact: u8,
}

fn hash_encode(t: &[u8; 20]) -> String {
    let encoded: String = t.iter().map(|b| format!("%{:02x}", b)).collect();
    //eprintln!("{encoded}");
    encoded
}

#[derive(Debug, Clone, Deserialize)]
struct TrackerResponse {
    interval: u32,

    #[serde(with = "serde_bytes")]
    peers: Vec<u8>,
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

            let tracker_url = reqwest::Url::parse(&format!(
                "{}?info_hash={}",
                torrent.announce,
                hash_encode(&torrent.info.hash()?)
            ))?;

            let client = Client::new().get(tracker_url).query(&TrackerRequest {
                //info_hash: hashed_info[..].try_into()?,
                peer_id: "00112233445566778899".to_owned(),
                port: 6881,
                uploaded: 0,
                downloaded: 0,
                left: torrent.info.length,
                compact: 1,
            });

            //eprintln!("{:?}", client);

            let response = client.send().await.context("Tracker request builder")?;

            //eprintln!("{}", response.status());
            //println!("{}", response.text().await?);

            let response = serde_bencode::from_bytes::<TrackerResponse>(&response.bytes().await?)
                .context("Decoding response")?;

            //eprintln!("{response:?}");
            // let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
            let peers: Vec<_> = response
                .peers
                .chunks_exact(6)
                .map(|c| {
                    SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::new(c[0], c[1], c[2], c[3])),
                        u16::from_be_bytes([c[4], c[5]]),
                    )
                })
                .collect();

            peers.iter().for_each(|p| println!("{p:?}"));
        }

        Command::Handshake { path, peer } => {
            eprintln!("{path:?} {peer:?}");
            let content = fs::read(path).context("Reading torrent file")?;
            let torrent =
                serde_bencode::from_bytes::<TorrentFile>(&content).context("parse torrent file")?;

            let mut tcp_peer = TcpStream::connect(peer)
                .await
                .context("Connecting to peer")?;

            let hs = Handshake::new(torrent.info.hash()?, b"00112233445566778899".to_owned());
            tcp_peer.write_all(&hs.to_bytes()).await?;

            let mut buf = [0; HANDSHAKE_LEN];
            tcp_peer.read_exact(&mut buf).await?;

            //eprintln!("{buf:?}");

            let hs_resp = Handshake::from_bytes(&buf)?;
            println!("Peer ID: {}", hex::encode(hs_resp.peer_id));
        }
    }

    Ok(())
}
