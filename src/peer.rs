
use bytes::{Buf, BufMut, Bytes, BytesMut};



#[derive(Debug)]
pub struct Handshake {
    
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

const BITTORRENT: &[u8;19] = b"BitTorrent protocol";
pub const HANDSHAKE_LEN: usize = 1+ BITTORRENT.len() + 8 + 20 + 20;

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Handshake {
            info_hash: info_hash,
            peer_id: peer_id,
        }
    }

    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(1+19+8+20+20);

        buf.put_u8(BITTORRENT.len() as u8);
        buf.put_slice(BITTORRENT);
        buf.put_bytes(0, 8); // reserved
        buf.put_slice(&self.info_hash);
        buf.put_slice(&self.peer_id);

        buf.freeze()

    }

    pub fn from_bytes(mut buf: &[u8]) -> anyhow::Result<Self> {
        let len = buf[0] as usize;
        buf.advance(1+len+8);

        Ok(Handshake { info_hash: buf[..20].try_into()?, peer_id: buf[20..].try_into()? })
    }
}
