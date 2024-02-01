use anyhow::Context;
use bytes::{Buf, BufMut, Bytes, BytesMut};

use std::net::SocketAddr;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use tokio::net::TcpStream;

use crate::torrent::TorrentFile;

#[derive(Debug)]
struct Handshake {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

const BITTORRENT: &[u8; 19] = b"BitTorrent protocol";
const HANDSHAKE_LEN: usize = 1 + BITTORRENT.len() + 8 + 20 + 20;

impl Handshake {
    fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Handshake {
            info_hash: info_hash,
            peer_id: peer_id,
        }
    }

    fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(1 + 19 + 8 + 20 + 20);

        buf.put_u8(BITTORRENT.len() as u8);
        buf.put_slice(BITTORRENT);
        buf.put_bytes(0, 8); // reserved
        buf.put_slice(&self.info_hash);
        buf.put_slice(&self.peer_id);

        buf.freeze()
    }

    fn from_bytes(mut buf: &[u8]) -> anyhow::Result<Self> {
        let len = buf[0] as usize;
        buf.advance(1 + len + 8);

        Ok(Handshake {
            info_hash: buf[..20].try_into()?,
            peer_id: buf[20..].try_into()?,
        })
    }
}

pub struct Peer {
    pub remote_addr: SocketAddr,
    pub local_id: [u8; 20],
    pub remote_id: [u8; 20],
    stream: TcpStream,
}

impl Peer {
    pub async fn connect(addr: SocketAddr, torrent: &TorrentFile) -> anyhow::Result<Self> {
        let mut tcp_peer = TcpStream::connect(addr)
            .await
            .context("Connecting to peer")?;

        let local_id = b"00112233445566778899";

        let hs = Handshake::new(torrent.info.hash()?, local_id.to_owned());
        tcp_peer.write_all(&hs.to_bytes()).await?;

        let mut buf = [0; HANDSHAKE_LEN];
        tcp_peer.read_exact(&mut buf).await?;

        //eprintln!("{buf:?}");

        let hs_resp = Handshake::from_bytes(&buf)?;

        Ok(Peer {
            remote_addr: addr,
            stream: tcp_peer,
            local_id: local_id.to_owned(),
            remote_id: hs_resp.peer_id,
        })
    }
}
