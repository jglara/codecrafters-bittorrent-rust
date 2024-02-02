use anyhow::{anyhow, Context};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use sha1::{Digest, Sha1};


use std::collections::VecDeque;
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
        let mut buf = BytesMut::with_capacity(HANDSHAKE_LEN);

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

#[derive(Debug)]
struct Message<'a> {
    length: usize,
    kind: MessageType,
    payload: MessagePayload<'a>,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
enum MessageType {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have,
    Bitfield,
    Request,
    Piece,
    Cancel,
    Ping = 255,
}

#[derive(Debug)]
enum MessagePayload<'a> {
    None,
    Have(u32),
    Bitfield(&'a [u8]),
    PieceInfo {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        piece: &'a [u8],
    },
}

#[derive(Debug, Clone, Copy)]
enum PeerState {
    Choked,
    Unchoked,
}

impl<'a> Message<'a> {
    fn status(kind: MessageType) -> Self {
        Message {
            length: 1,
            kind: kind,
            payload: MessagePayload::None,
        }
    }

    fn request(piece_id: usize, block_begin: usize, block_length: usize) -> Self {
        Message {
            length: 1 + 4 * 3,
            kind: MessageType::Request,
            payload: MessagePayload::PieceInfo {
                index: piece_id as u32,
                begin: block_begin as u32,
                length: block_length as u32,
            },
        }
    }

    fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(self.length);
        buf.put_u32(self.length as u32);
        buf.put_u8(self.kind as u8);
        match &self.payload {
            MessagePayload::None => {}
            MessagePayload::Bitfield(bf) => buf.put_slice(bf),
            MessagePayload::Have(index) => buf.put_u32(*index),
            MessagePayload::PieceInfo {
                index,
                begin,
                length,
            } => {
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
            }
            MessagePayload::Piece {
                index,
                begin,
                piece,
            } => {
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_slice(piece)
            }
        }

        buf.freeze()
    }

    fn from_bytes(mut buf: &'a [u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(buf.len() >= 5);

        let len = buf.get_u32() as usize;
        let kind = buf.get_u8();

        let (kind, payload) = match kind {
            0 => (MessageType::Choke, MessagePayload::None),
            1 => (MessageType::Unchoke, MessagePayload::None),
            2 => (MessageType::Interested, MessagePayload::None),
            3 => (MessageType::NotInterested, MessagePayload::None),
            4 => (MessageType::Have, MessagePayload::Have(buf.get_u32())),
            5 => (MessageType::Bitfield, MessagePayload::Bitfield(&buf[..])),
            6 => {
                anyhow::ensure!(buf.len() == 4 * 3);
                (
                    MessageType::Request,
                    MessagePayload::PieceInfo {
                        index: buf.get_u32(),
                        begin: buf.get_u32(),
                        length: buf.get_u32(),
                    },
                )
            }
            7 => {
                anyhow::ensure!(buf.len() > 4 * 2);
                (
                    MessageType::Piece,
                    MessagePayload::Piece {
                        index: buf.get_u32(),
                        begin: buf.get_u32(),
                        piece: &buf[..],
                    },
                )
            }
            8 => {
                anyhow::ensure!(buf.len() == 4 * 3);
                (
                    MessageType::Cancel,
                    MessagePayload::PieceInfo {
                        index: buf.get_u32(),
                        begin: buf.get_u32(),
                        length: buf.get_u32(),
                    },
                )
            }
            _ => anyhow::bail!("Invalid message type"),
        };

        Ok(Message {
            length: len,
            kind: kind,
            payload: payload,
        })
    }
}

pub struct Peer {
    pub remote_addr: SocketAddr,
    pub local_id: [u8; 20],
    pub remote_id: [u8; 20],
    pub pieces_bitfield: Vec<u8>,
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
            pieces_bitfield: vec![],
        })
    }

    async fn recv_message<'a>(&mut self, buf: &'a mut [u8]) -> anyhow::Result<Message<'a>> {
        self.stream.read_exact(&mut buf[..4]).await?; // length
                                                      //eprintln!("Read {:?}", &buf[..5]);
        let len = u32::from_be_bytes(buf[..4].try_into()?);
        if len == 0 {
            return Ok(Message {
                length: 0,
                kind: MessageType::Ping,
                payload: MessagePayload::None,
            });
        }


        self.stream
            .read_exact(&mut buf[4..4 + len as usize])
            .await?; // type + payload
        let msg = Message::from_bytes(&buf[..4 + len as usize])?;

        eprintln!("Received: {:?}", msg.kind);

        Ok(msg)
    }

    pub async fn recv_bitfield(&mut self) -> anyhow::Result<()> {
        let mut buf = [0; 1028];
        let msg = self.recv_message(&mut buf).await?;
        if let MessagePayload::Bitfield(bf) = msg.payload {
            self.pieces_bitfield.extend_from_slice(bf);
            Ok(())
        } else {
            Err(anyhow!("Invalid msg {:?}", msg))
        }
    }

    fn requests(piece_id: usize, block_size: usize, total_len: usize) -> VecDeque<Message<'static>>
    {
        (0..(total_len / block_size)+1).scan(0, |cur_offset, _| {
            if *cur_offset < total_len {
            let (block_begin, block_length) = (
                *cur_offset,
                std::cmp::min(block_size, total_len - *cur_offset),
            );
            let msg = Message::request(piece_id, block_begin, block_length);
            *cur_offset += block_length;
            Some(msg)
        } else {
            None
        }
        }).collect()
    }

    pub async fn download_piece(
        &mut self,
        piece_index: usize,
        piece_length: usize,
        piece_hash: &[u8; 20],
    ) -> anyhow::Result<Bytes> {
        const BLOCK_SIZE: usize = 16 * 1024; // 16 Kb
        let mut buf = [0; 1024 + BLOCK_SIZE];
        let mut piece_buf: BytesMut = BytesMut::with_capacity(piece_length);
        let mut pending_piece_offset = 0;
        let mut pending_requests = Peer::requests(piece_index, BLOCK_SIZE, piece_length);
        const PIPELINED_REQUESTS: u32 = 5;

        eprintln!("Downloading piece {piece_index} len {piece_length}");

        // Send interested message
        let msg = Message::status(MessageType::Interested);
        eprintln!("Sending {msg:?}");
        self.stream.write_all(&msg.to_bytes()).await?;

        let mut state = PeerState::Choked;
        while pending_piece_offset < piece_length {
            let msg = self.recv_message(&mut buf).await?;

            if msg.length == 0 {
                continue;
            };

            match (state, msg.kind, msg.payload) {
                (PeerState::Choked, MessageType::Choke, _) => {
                    // change state to choked
                    state = PeerState::Choked;
                },
                (PeerState::Choked, MessageType::Unchoke, _) => {
                    // Send requests
                    for _ in 0..PIPELINED_REQUESTS {
                        if let Some(msg) = pending_requests.pop_front() {
                            eprintln!("Sending {msg:?}");
                            self.stream.write_all(&msg.to_bytes()).await?;
                        } else {
                            break;
                        }
                    }
                    
                    // change state to unchoked
                    state = PeerState::Unchoked;
                }
                (PeerState::Unchoked, MessageType::Choke, _) => {
                    // change state to choked
                    state = PeerState::Choked;
                }
                (
                    PeerState::Unchoked,
                    MessageType::Piece,
                    MessagePayload::Piece {
                        index,
                        begin,
                        piece,
                    },
                ) => {
                    eprintln!("Piece {index} offset={begin} len={}", piece.len());
                    anyhow::ensure!(index as usize == piece_index);
                    anyhow::ensure!(begin as usize == pending_piece_offset);
                    piece_buf.put(piece);

                    pending_piece_offset += piece.len();

                    // Send next request
                    if let Some(msg) = pending_requests.pop_front() {
                        eprintln!("Sending {msg:?}");
                        self.stream.write_all(&msg.to_bytes()).await?;
                    }
                    
                }
                (_, k, _) => anyhow::bail!("unexpected msg {:?} state {state:?}", k),
            }
        }

        // check hash
        let mut hasher = Sha1::new();
        hasher.update(&piece_buf[..]);
        let hashed_info = hasher.finalize();

        if &hashed_info[..] != piece_hash {
            Err(anyhow!("Invalid hash of received piece {:?} {:?}", hashed_info, piece_hash))
        } else {
            Ok(piece_buf.freeze())
        }
    }
}
