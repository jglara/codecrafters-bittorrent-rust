use anyhow::{anyhow, Context};
use clap::Parser;
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json;
use sha1::{Digest, Sha1};

use clap;
use serde_bencode;
use std::fs;
use std::path::PathBuf;

fn parse_bencoded_string(input: &str) -> Option<(serde_json::Value, &str)> {
    input
        .split_once(":")
        .and_then(|(len, rest)| Some((len.parse::<usize>().ok()?, rest)))
        .map(|(len, rest)| ((&rest[..len]).into(), &rest[len..]))
}

fn parse_bencoded_i64(input: &str) -> Option<(serde_json::Value, &str)> {
    input
        .strip_prefix('i')
        .and_then(|rest| rest.split_once('e'))
        .and_then(|(s, rest)| Some((s.parse::<i64>().ok()?.into(), rest)))
}

fn parse_bencoded_value(input: &str) -> Option<(serde_json::Value, &str)> {
    match input.chars().next() {
        Some('i') => parse_bencoded_i64(input),
        Some('0'..='9') => parse_bencoded_string(input),
        Some('l') => {
            //eprintln!("parsing {input:?}");
            let mut input = &input[1..];
            let mut vec = vec![];
            while input.chars().next()? != 'e' {
                let (v, rem) = parse_bencoded_value(input)?;
                vec.push(v);
                input = rem;
            }
            Some((vec.into(), &input[1..]))
        }
        Some('d') => {
            let mut input = &input[1..];
            let mut d = serde_json::Map::new();
            while input.chars().next()? != 'e' {
                let (key, rest) = parse_bencoded_string(input)?;
                let (val, rest) = parse_bencoded_value(rest)?;
                if let serde_json::Value::String(key) = key {
                    d.insert(key, val);
                }
                input = rest;
            }
            Some((d.into(), &input[1..]))
        }
        _ => None,
    }
}

fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    if let Some((v, _)) = parse_bencoded_value(encoded_value) {
        v
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TorrentFile {
    announce: String,
    info: TorrentInfo,
}

fn main() -> anyhow::Result<()> {
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

            let info = serde_bencode::to_bytes(&torrent.info)?;
            let mut hasher = Sha1::new();
            hasher.update(&info);
            let hashed_info = hasher.finalize();

            println!("Tracker URL: {}", torrent.announce);
            println!("Length: {}", torrent.info.length);
            println!("Info Hash: {}", hex::encode(hashed_info));
            println!("Piece Length: {}", torrent.info.piece_length);
            println!("Piece Hashes:");
            torrent
                .info
                .piece_hashes()?
                .iter()
                .for_each(|h| println!("{}", hex::encode(h)));
        }
    }

    Ok(())
}
