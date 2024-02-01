use serde::{Deserialize, Serialize};
use anyhow::Context;

use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use reqwest::Client;

use crate::torrent::TorrentFile;

#[derive(Debug, Clone, Serialize)]
struct TrackerRequest {
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

pub struct Tracker {
    peer_id: String,
    port: u16,
}

impl Tracker {
    pub fn new() -> Self {
        Tracker { peer_id: "00112233445566778899".to_owned(), port: 6881 }
    }

    pub async fn req_peers(&self, torrent: &TorrentFile) -> anyhow::Result<Vec<SocketAddr>> {
        
        let tracker_url = reqwest::Url::parse(&format!(
            "{}?info_hash={}",
            torrent.announce,
            hash_encode(&torrent.info.hash()?)
        ))?;

        let client = Client::new().get(tracker_url).query(&TrackerRequest {
            peer_id: self.peer_id.to_owned(),
            port: self.port,
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

        eprintln!("interval {:?}", response.interval);
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

        Ok(peers)

    }
}
