use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Context};
use sha1::{Digest, Sha1};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentInfo {
    pub name: String,

    #[serde(rename = "piece length")]
    pub piece_length: usize,

    #[serde(with = "serde_bytes")]
    pub pieces: Vec<u8>,

    pub length: usize,
}

impl TorrentInfo {
    pub fn piece_hashes(&self) -> anyhow::Result<Vec<&[u8; 20]>> {
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

    pub fn hash(&self) -> anyhow::Result<[u8; 20]> {
        let info = serde_bencode::to_bytes(&self)?;
        let mut hasher = Sha1::new();
        hasher.update(&info);
        let hashed_info = hasher.finalize();

        hashed_info[..].try_into().map_err(|e| anyhow!("{}", e))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFile {
    pub announce: String,
    pub info: TorrentInfo,
}
